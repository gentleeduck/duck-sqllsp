//! `textDocument/linkedEditingRange` handler.
//!
//! When the cursor sits on an identifier that has repeats inside the
//! same statement (e.g. an alias mentioned several times in WHERE /
//! ORDER BY / JOIN ON), VS Code lets the user edit every occurrence
//! at once. Returns the list of byte ranges all sharing the same
//! identifier text, scoped to the enclosing statement (semicolon-
//! bounded). Cross-statement co-edits don't make sense for SQL --
//! a table name in two different SELECTs isn't the same binding.

use crate::handlers::{position, references};
use crate::state::ServerState;
use ropey::Rope;
use tower_lsp::lsp_types::{LinkedEditingRangeParams, LinkedEditingRanges, Position, Range};

pub fn run(state: &ServerState, params: LinkedEditingRangeParams) -> Option<LinkedEditingRanges> {
  let uri = &params.text_document_position_params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("linked_editing", uri);
  let doc = state.documents.get(uri)?;
  let offset = position::to_offset(&doc.rope, params.text_document_position_params.position);
  let pos: usize = u32::from(offset) as usize;
  let token = token_at(&doc.text, pos)?;
  if token.is_empty() {
    return None;
  }

  // Statement bounds: previous unquoted `;` (exclusive) and next
  // unquoted `;` (exclusive). Keeps co-edits scoped to one statement.
  let (stmt_start, stmt_end) = statement_bounds(&doc.text, pos);
  let stmt = &doc.text[stmt_start..stmt_end];

  let mut ranges: Vec<Range> = Vec::new();
  for (s, e) in references::find_word_occurrences(stmt, &token) {
    let abs_s = stmt_start + s;
    let abs_e = stmt_start + e;
    ranges.push(Range { start: byte_to_position(&doc.rope, abs_s), end: byte_to_position(&doc.rope, abs_e) });
  }
  // Less than 2 occurrences -> nothing to link.
  if ranges.len() < 2 {
    return None;
  }
  Some(LinkedEditingRanges { ranges, word_pattern: None })
}

fn token_at(src: &str, pos: usize) -> Option<String> {
  let bytes = src.as_bytes();
  let pos = pos.min(src.len());
  let mut start = pos;
  while start > 0 && is_word(bytes[start - 1] as char) {
    start -= 1;
  }
  let mut end = pos;
  while end < bytes.len() && is_word(bytes[end] as char) {
    end += 1;
  }
  if start == end {
    return None;
  }
  Some(src[start..end].to_string())
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// Find the (start, end) byte range of the statement enclosing `pos`.
/// `;` inside strings is ignored (`'a;b'` is one literal).
fn statement_bounds(src: &str, pos: usize) -> (usize, usize) {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut in_single = false;
  let mut in_double = false;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < pos {
    let c = bytes[i];
    if !in_double && c == b'\'' && (i == 0 || bytes[i - 1] != b'\\') {
      in_single = !in_single;
    } else if !in_single && c == b'"' {
      in_double = !in_double;
    } else if !in_single && !in_double && c == b';' {
      start = i + 1;
    }
    i += 1;
  }
  let mut end = n;
  let mut in_single = false;
  let mut in_double = false;
  let mut j = pos;
  while j < n {
    let c = bytes[j];
    if !in_double && c == b'\'' && (j == 0 || bytes[j - 1] != b'\\') {
      in_single = !in_single;
    } else if !in_single && c == b'"' {
      in_double = !in_double;
    } else if !in_single && !in_double && c == b';' {
      end = j;
      break;
    }
    j += 1;
  }
  (start, end)
}

fn byte_to_position(rope: &Rope, byte: usize) -> Position {
  let byte = byte.min(rope.len_bytes());
  let line = rope.byte_to_line(byte);
  let line_start_byte = rope.line_to_byte(line);
  let line_slice = rope.line(line);
  let mut utf16 = 0u32;
  let mut bytes_seen = 0usize;
  let bytes_in_line = byte.saturating_sub(line_start_byte);
  for c in line_slice.chars() {
    if bytes_seen >= bytes_in_line {
      break;
    }
    utf16 += c.len_utf16() as u32;
    bytes_seen += c.len_utf8();
  }
  Position { line: line as u32, character: utf16 }
}
