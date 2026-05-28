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
  // a preceding blank line or the previous statement's terminator.
  let leading_ws = raw.len() - raw.trim_start().len();
  let trimmed_start = start + leading_ws;
  let trimmed_end = trimmed_start + chunk.len();
  out.push((chunk, TextRange::new(TextSize::from(trimmed_start as u32), TextSize::from(trimmed_end as u32))));
}
