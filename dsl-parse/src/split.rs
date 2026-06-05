//! Split a SQL document on top-level semicolons.
//!
//! Why we slice ourselves rather than letting the parser do it: `sqlparser`
//! aborts on the first syntax error, which would poison the rest of the
//! file. By slicing first and feeding each piece in independently we
//! contain failures to a single statement and keep editing other parts of
//! the file fully analysed.
//!
//! Edge cases handled:
//!   - Single-quoted strings: semicolons inside are part of the literal.
//!   - Double-quoted identifiers: same.
//!   - Postgres dollar-quoted blocks: `$tag$ ... $tag$` (any tag, including
//!     the empty tag `$$ ... $$`).
//!   - Backslash escapes inside quoted strings.
//!
//! Line comments (`--` to end-of-line) and block comments (`/* ... */`,
//! including nested PG-style blocks) are stripped *for the purpose of
//! finding statement boundaries*. A `;` inside a comment must not
//! split the statement, otherwise the body fragment after `;` parses
//! as broken SQL and produces a sql000 syntax error.

use text_size::{TextRange, TextSize};

/// Returns `(trimmed_chunk, range_in_source)` for each top-level statement.
/// Whitespace-only chunks are dropped.
pub fn split_statements(src: &str) -> Vec<(String, TextRange)> {
  let mut out: Vec<(String, TextRange)> = Vec::new();
  let bytes = src.as_bytes();
  let mut start = 0usize;
  let mut i = 0usize;
  let mut in_single = false;
  let mut in_double = false;
  let mut dollar_tag: Option<String> = None;
  let mut block_depth: u32 = 0;

  while i < bytes.len() {
    let c = bytes[i] as char;

    // Block comment open / close (nestable, PG-flavoured).
    if !in_single && !in_double && dollar_tag.is_none() {
      if block_depth > 0 {
        if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
          block_depth -= 1;
          i += 2;
          continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
          block_depth += 1;
          i += 2;
          continue;
        }
        i += 1;
        continue;
      }
      if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
        block_depth = 1;
        i += 2;
        continue;
      }
      // Line comment: skip to end of line.
      if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
        while i < bytes.len() && bytes[i] != b'\n' {
          i += 1;
        }
        continue;
      }
    }

    if let Some(tag) = &dollar_tag {
      let closer = format!("${tag}$");
      if src[i..].starts_with(&closer) {
        i += closer.len();
        dollar_tag = None;
        continue;
      }
      i += 1;
      continue;
    }

    if !in_single && !in_double && c == '$' {
      // Try to read an opening dollar tag: $ident$ or $$.
      let rest = &src[i + 1..];
      if let Some(end) = rest.find('$') {
        let tag = &rest[..end];
        if tag.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
          dollar_tag = Some(tag.to_string());
          i += 1 + end + 1; // skip past $tag$
          continue;
        }
      }
    }

    if !in_double && c == '\'' && (i == 0 || bytes[i - 1] != b'\\') {
      in_single = !in_single;
    } else if !in_single && c == '"' && (i == 0 || bytes[i - 1] != b'\\') {
      in_double = !in_double;
    } else if !in_single && !in_double && c == ';' {
      push_chunk(src, start, i, &mut out);
      start = i + 1;
    }
    i += 1;
  }

  push_chunk(src, start, src.len(), &mut out);
  out
}

fn push_chunk(src: &str, start: usize, end: usize, out: &mut Vec<(String, TextRange)>) {
  let raw = &src[start..end];
  let chunk = raw.trim().to_string();
  if chunk.is_empty() {
    return;
  }
  // The raw slice spans `start..end` (semicolon-bounded), but the
  // trimmed chunk excludes leading/trailing whitespace and the
  // trailing semicolon. The range must follow the trimmed chunk so
  // diagnostics and inlay hints anchor on the actual statement, not
  // a preceding blank line, the previous statement's terminator, or
  // a leading comment block (the codelens above-line anchor must
  // sit above the SQL, not above any preceding doc-comment).
  let trimmed_start = start + skip_leading_ws_and_comments(raw);
  let trimmed_end = trimmed_start + (end - trimmed_start) - trailing_ws_bytes(&src[trimmed_start..end]);
  // Also re-slice the chunk to match the new range so the stored
  // chunk string excludes the leading comment block consistently.
  let trimmed_chunk = src[trimmed_start..trimmed_end].trim().to_string();
  if trimmed_chunk.is_empty() {
    return;
  }
  // Some callers expect the chunk to end exactly where its range
  // ends, so use the chunk's length for the right edge after the
  // trim_end above strips trailing whitespace.
  let final_end = trimmed_start + trimmed_chunk.len();
  out.push((trimmed_chunk, TextRange::new(TextSize::from(trimmed_start as u32), TextSize::from(final_end as u32))));
}

/// Count leading bytes of `s` that are whitespace, `-- line comments`,
/// or `/* block comments */`. Used to advance the statement range so
/// the editor anchors codelens / inlay hints on the SQL itself rather
/// than on a comment block that documents the statement.
fn skip_leading_ws_and_comments(s: &str) -> usize {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  loop {
    let before = i;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      let mut depth = 1u32;
      i += 2;
      while i + 1 < n && depth > 0 {
        if bytes[i] == b'/' && bytes[i + 1] == b'*' {
          depth += 1;
          i += 2;
        } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
          depth -= 1;
          i += 2;
        } else {
          i += 1;
        }
      }
      continue;
    }
    if i == before {
      return i;
    }
  }
}

fn trailing_ws_bytes(s: &str) -> usize {
  s.len() - s.trim_end().len()
}

#[cfg(test)]
mod tests_r4 {
  use super::*;
  #[test]
  fn r4_stmt_range_skips_leading_line_comment() {
    // CYCLE 4: stmt.range used to include the leading `-- comment\n`
    // which made codelens anchor above the comment, not the SQL.
    // Now skipped.
    let src = "-- header comment\nSELECT 1;";
    let chunks = split_statements(src);
    assert_eq!(chunks.len(), 1);
    let (chunk, range) = &chunks[0];
    assert_eq!(chunk.trim(), "SELECT 1");
    let s: u32 = range.start().into();
    let select_pos = src.find("SELECT").unwrap() as u32;
    assert_eq!(s, select_pos, "range should start at SELECT, not comment");
  }

  #[test]
  fn r4_stmt_range_skips_block_comment() {
    let src = "/* block doc */\nSELECT 1;";
    let chunks = split_statements(src);
    let (_, range) = &chunks[0];
    let s: u32 = range.start().into();
    let select_pos = src.find("SELECT").unwrap() as u32;
    assert_eq!(s, select_pos);
  }

  #[test]
  fn r4_stmt_range_skips_multi_comment_block() {
    let src = "-- doc1\n-- doc2\n/* doc3 */\nSELECT * FROM users;";
    let chunks = split_statements(src);
    let (_, range) = &chunks[0];
    let s: u32 = range.start().into();
    let select_pos = src.find("SELECT").unwrap() as u32;
    assert_eq!(s, select_pos);
  }
}
