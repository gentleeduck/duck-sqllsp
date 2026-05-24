//! `textDocument/foldingRange` handler.
//!
//! Surfaces foldable line ranges so editors can collapse:
//!   * Top-level statements ending in `;` that span multiple lines.
//!   * Parenthesised blocks (CREATE TABLE column lists, INSERT VALUES
//!     tuples, function arg lists wrapping onto multiple lines).
//!   * PL/pgSQL BEGIN..END blocks.
//!   * Multi-line block comments `/* ... */`.
//!
//! Pure text-scan -- doesn't depend on the parser succeeding, so it
//! still works on half-written SQL the user is in the middle of typing.

use crate::state::ServerState;
use ropey::Rope;
use tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams};

pub fn run(state: &ServerState, params: FoldingRangeParams) -> Option<Vec<FoldingRange>> {
  let uri = &params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("folding_range", uri);
  let doc = state.documents.get(uri)?;

  let mut ranges: Vec<FoldingRange> = Vec::new();

  push_paren_blocks(&doc.text, &doc.rope, &mut ranges);
  push_block_comments(&doc.text, &doc.rope, &mut ranges);
  push_begin_end_blocks(&doc.text, &doc.rope, &mut ranges);
  push_statement_blocks(&doc.text, &doc.rope, &mut ranges);

  if ranges.is_empty() { None } else { Some(ranges) }
}

/// Every balanced top-level `(...)` that spans more than one line gets
/// a fold from the `(` line to the line *before* the `)` (LSP convention
/// keeps the closing paren visible).
fn push_paren_blocks(src: &str, rope: &Rope, out: &mut Vec<FoldingRange>) {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut stack: Vec<usize> = Vec::new();
  let mut i = 0;
  while i < n {
    let c = bytes[i] as char;
    // Skip strings + comments + dollar-quoted to avoid false matches.
    if c == '\'' {
      i = skip_single_quoted(bytes, i);
      continue;
    }
    if c == '"' {
      i = skip_double_quoted(bytes, i);
      continue;
    }
    if c == '-' && i + 1 < n && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    if c == '/' && i + 1 < n && bytes[i + 1] == b'*' {
      i = skip_block_comment(bytes, i);
      continue;
    }
    if c == '$' {
      if let Some(end) = skip_dollar_quoted(bytes, i) {
        i = end;
        continue;
      }
    }
    if c == '(' {
      stack.push(i);
    } else if c == ')' {
      if let Some(open) = stack.pop() {
        let open_line = byte_line(rope, open);
        let close_line = byte_line(rope, i);
        if close_line > open_line {
          out.push(FoldingRange {
            start_line: open_line,
            start_character: None,
            end_line: close_line.saturating_sub(1),
            end_character: None,
            kind: Some(FoldingRangeKind::Region),
            collapsed_text: None,
          });
        }
      }
    }
    i += 1;
  }
}

fn push_block_comments(src: &str, rope: &Rope, out: &mut Vec<FoldingRange>) {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i + 1 < n {
    // Skip strings / dollar-quoted so a /* inside a string is not
    // mistaken for a comment opener.
    let c = bytes[i] as char;
    if c == '\'' {
      i = skip_single_quoted(bytes, i);
      continue;
    }
    if c == '"' {
      i = skip_double_quoted(bytes, i);
      continue;
    }
    if c == '$' {
      if let Some(end) = skip_dollar_quoted(bytes, i) {
        i = end;
        continue;
      }
    }
    if c == '/' && bytes[i + 1] == b'*' {
      let start = i;
      let end = skip_block_comment(bytes, i);
      let start_line = byte_line(rope, start);
      let end_line = byte_line(rope, end.saturating_sub(1));
      if end_line > start_line {
        out.push(FoldingRange {
          start_line,
          start_character: None,
          end_line,
          end_character: None,
          kind: Some(FoldingRangeKind::Comment),
          collapsed_text: None,
        });
      }
      i = end;
      continue;
    }
    i += 1;
  }
}

/// PL/pgSQL `BEGIN ... END` blocks. The BEGIN keyword may be followed
/// by `;` (start of a transaction) -- those are 1-line and don't fold.
/// We track nested BEGIN depth so an outer block's fold doesn't get
/// claimed by an inner END.
fn push_begin_end_blocks(src: &str, rope: &Rope, out: &mut Vec<FoldingRange>) {
  let upper = src.to_ascii_uppercase();
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut stack: Vec<usize> = Vec::new();
  let mut i = 0;
  while i < n {
    // Skip strings / comments / dollar-quoted regions when tracking
    // keywords -- a `BEGIN` inside a string is not a real block.
    // But: PL/pgSQL bodies live INSIDE `$$ ... $$`, and that's where
    // the BEGIN/END blocks we want to fold sit. So we DO want to
    // descend into dollar-quoted regions but skip plain strings and
    // comments.
    let c = bytes[i] as char;
    if c == '\'' {
      i = skip_single_quoted(bytes, i);
      continue;
    }
    if c == '-' && i + 1 < n && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    if c == '/' && i + 1 < n && bytes[i + 1] == b'*' {
      i = skip_block_comment(bytes, i);
      continue;
    }
    // Identifier-aligned BEGIN / END detection (word-bounded).
    if !is_word_start(c) || (i > 0 && is_word_cont(bytes[i - 1] as char)) {
      i += 1;
      continue;
    }
    let mut j = i;
    while j < n && is_word_cont(bytes[j] as char) {
      j += 1;
    }
    let word = &upper[i..j];
    match word {
      "BEGIN" => stack.push(i),
      "END" => {
        if let Some(open) = stack.pop() {
          let open_line = byte_line(rope, open);
          let close_line = byte_line(rope, i);
          if close_line > open_line {
            out.push(FoldingRange {
              start_line: open_line,
              start_character: None,
              end_line: close_line.saturating_sub(1),
              end_character: None,
              kind: Some(FoldingRangeKind::Region),
              collapsed_text: None,
            });
          }
        }
      },
      _ => {},
    }
    i = j;
  }
}

/// Each top-level statement (terminated by `;`) that spans more than
/// one line gets a fold. Keeps very long migration files navigable
/// even when no other foldable construct is present.
fn push_statement_blocks(src: &str, rope: &Rope, out: &mut Vec<FoldingRange>) {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut stmt_start = 0usize;
  let mut i = 0;
  while i < n {
    let c = bytes[i] as char;
    if c == '\'' {
      i = skip_single_quoted(bytes, i);
      continue;
    }
    if c == '"' {
      i = skip_double_quoted(bytes, i);
      continue;
    }
    if c == '$' {
      if let Some(end) = skip_dollar_quoted(bytes, i) {
        i = end;
        continue;
      }
    }
    if c == ';' {
      let start = first_nonws(src, stmt_start);
      let start_line = byte_line(rope, start);
      let end_line = byte_line(rope, i);
      if end_line > start_line {
        out.push(FoldingRange {
          start_line,
          start_character: None,
          end_line: end_line.saturating_sub(0),
          end_character: None,
          kind: Some(FoldingRangeKind::Region),
          collapsed_text: None,
        });
      }
      stmt_start = i + 1;
    }
    i += 1;
  }
}

fn first_nonws(src: &str, mut i: usize) -> usize {
  let bytes = src.as_bytes();
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}

fn skip_single_quoted(bytes: &[u8], i: usize) -> usize {
  let n = bytes.len();
  let mut j = i + 1;
  while j < n {
    if bytes[j] == b'\'' {
      if j + 1 < n && bytes[j + 1] == b'\'' {
        j += 2;
        continue;
      }
      return j + 1;
    }
    j += 1;
  }
  n
}

fn skip_double_quoted(bytes: &[u8], i: usize) -> usize {
  let n = bytes.len();
  let mut j = i + 1;
  while j < n && bytes[j] != b'"' {
    j += 1;
  }
  (j + 1).min(n)
}

fn skip_block_comment(bytes: &[u8], i: usize) -> usize {
  let n = bytes.len();
  let mut j = i + 2;
  while j + 1 < n && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
    j += 1;
  }
  (j + 2).min(n)
}

fn skip_dollar_quoted(bytes: &[u8], i: usize) -> Option<usize> {
  let n = bytes.len();
  if i >= n || bytes[i] != b'$' {
    return None;
  }
  let mut j = i + 1;
  while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
    j += 1;
  }
  if j >= n || bytes[j] != b'$' {
    return None;
  }
  let tag = &bytes[i..=j];
  let body_start = j + 1;
  let mut k = body_start;
  while k + tag.len() <= n {
    if &bytes[k..k + tag.len()] == tag {
      return Some(k + tag.len());
    }
    k += 1;
  }
  Some(n)
}

fn is_word_start(c: char) -> bool {
  c.is_alphabetic() || c == '_'
}
fn is_word_cont(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn byte_line(rope: &Rope, byte: usize) -> u32 {
  let byte = byte.min(rope.len_bytes());
  rope.byte_to_line(byte) as u32
}
