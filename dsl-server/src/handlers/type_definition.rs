//! `textDocument/typeDefinition` handler.
//!
//! Jumps to the user-defined TYPE / DOMAIN definition of:
//!   * A `::<type>` cast target -- cursor on the type identifier.
//!   * A column reference -- looks up the column's declared type,
//!     then locates the matching `CREATE TYPE` / `CREATE DOMAIN`
//!     in the open buffer.
//!   * A bare type name (e.g. inside CREATE TABLE column declaration).
//!
//! Returns None when the type is built-in (no source location to jump
//! to) or when no matching definition exists in any open buffer.

use crate::handlers::position;
use crate::state::ServerState;
use ropey::Rope;
use tower_lsp::lsp_types::request::{GotoTypeDefinitionParams, GotoTypeDefinitionResponse};
use tower_lsp::lsp_types::{Location, Position, Range};

pub fn run(state: &ServerState, params: GotoTypeDefinitionParams) -> Option<GotoTypeDefinitionResponse> {
  let uri = params.text_document_position_params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("type_definition", &uri);
  let doc = state.documents.get(&uri)?;
  let offset = position::to_offset(&doc.rope, params.text_document_position_params.position);
  let pos: usize = u32::from(offset) as usize;
  let token = token_at(&doc.text, pos)?;

  // Prefer the explicit cast target: if the cursor sits on the type
  // after `::`, the token IS the type name.
  let cat = state.catalog.read();
  let lookup_type = if after_cast(&doc.text, pos) {
    token.clone()
  } else if let Some((_, t)) = cat.columns_named(&token).first() {
    t.data_type.clone()
  } else {
    token.clone()
  };

  // Walk every open buffer for `CREATE TYPE` / `CREATE DOMAIN <type>`.
  for (other_uri, other_doc) in state.documents.snapshot() {
    if let Some(range) = find_type_definition(&other_doc.text, &lookup_type) {
      return Some(GotoTypeDefinitionResponse::Scalar(Location {
        uri: other_uri,
        range: byte_range_to_lsp(&other_doc.rope, range),
      }));
    }
  }
  None
}

fn token_at(src: &str, pos: usize) -> Option<String> {
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

/// True when the cursor lies in a `<expr>::<cursor>` position.
fn after_cast(src: &str, pos: usize) -> bool {
  let bytes = src.as_bytes();
  let mut k = pos;
  while k > 0 && is_word(bytes[k - 1] as char) {
    k -= 1;
  }
  while k > 0 && bytes[k - 1].is_ascii_whitespace() {
    k -= 1;
  }
  k >= 2 && bytes[k - 1] == b':' && bytes[k - 2] == b':'
}

/// Locate the `CREATE TYPE <name>` / `CREATE DOMAIN <name>` declaration
/// in `src` and return the byte range of the name token.
fn find_type_definition(src: &str, name: &str) -> Option<(usize, usize)> {
  let upper = src.to_ascii_uppercase();
  for prefix in ["CREATE TYPE ", "CREATE DOMAIN "] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(prefix) {
      let after = from + rel + prefix.len();
      let bytes = src.as_bytes();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      let id_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.') {
        k += 1;
      }
      let id_end = k;
      if id_end > id_start {
        let candidate = &src[id_start..id_end];
        let bare = candidate.rsplit('.').next().unwrap_or(candidate);
        if bare.eq_ignore_ascii_case(name) {
          let local_start = id_start + (candidate.len() - bare.len());
          return Some((local_start, id_end));
        }
      }
      from = after;
    }
  }
  None
}

fn byte_range_to_lsp(rope: &Rope, range: (usize, usize)) -> Range {
  Range { start: byte_to_position(rope, range.0), end: byte_to_position(rope, range.1) }
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
