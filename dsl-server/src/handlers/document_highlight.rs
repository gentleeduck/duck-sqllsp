//! `textDocument/documentHighlight` handler.
//!
//! Highlights every whole-word occurrence of the identifier under the
//! cursor in the cursor's buffer. Unlike `references`, this is a
//! single-buffer view: clients use it for "let me see every other spot
//! this identifier appears in the file I'm editing right now". Strings,
//! comments and dollar-quoted bodies are skipped via the same scanner
//! that powers references / rename.

use crate::handlers::{position, references};
use crate::state::ServerState;
use ropey::Rope;
use tower_lsp::lsp_types::{DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams, Position, Range};

pub fn run(state: &ServerState, params: DocumentHighlightParams) -> Option<Vec<DocumentHighlight>> {
  let uri = &params.text_document_position_params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("document_highlight", uri);
  let doc = state.documents.get(uri)?;
  let offset = position::to_offset(&doc.rope, params.text_document_position_params.position);
  let token = token_at(&doc.text, offset)?;

  let mut out = Vec::new();
  for (s, e) in references::find_word_occurrences(&doc.text, &token) {
    out.push(DocumentHighlight {
      range: Range { start: byte_to_position(&doc.rope, s), end: byte_to_position(&doc.rope, e) },
      kind: Some(DocumentHighlightKind::TEXT),
    });
  }
  if out.is_empty() { None } else { Some(out) }
}

fn token_at(src: &str, offset: text_size::TextSize) -> Option<String> {
  let pos: usize = offset.into();
  let pos = pos.min(src.len());
  let bytes = src.as_bytes();
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
