//! `textDocument/rename` + `textDocument/prepareRename`.
//!
//! Buffer-scoped rename: every whole-word occurrence of the identifier
//! under the cursor is rewritten. Strings, comments, and dollar-quoted
//! bodies are skipped (see `references::find_word_occurrences`). For a
//! global rename across files we would need cross-file analysis -- not
//! in scope yet.

use crate::handlers::{position, references};
use crate::state::ServerState;
use ropey::Rope;
use std::collections::HashMap;
use tower_lsp::lsp_types::{
    PrepareRenameResponse, Position, Range, RenameParams, TextDocumentPositionParams,
    TextEdit, WorkspaceEdit,
};

pub fn prepare(
    state: &ServerState,
    params: TextDocumentPositionParams,
) -> Option<PrepareRenameResponse> {
    let doc = state.documents.get(&params.text_document.uri)?;
    let offset = position::to_offset(&doc.rope, params.position);
    let (start, end, token) = token_range(&doc.text, offset)?;
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: Range {
            start: byte_to_position(&doc.rope, start),
            end:   byte_to_position(&doc.rope, end),
        },
        placeholder: token,
    })
}

pub fn run(state: &ServerState, params: RenameParams) -> Option<WorkspaceEdit> {
    let uri = params.text_document_position.text_document.uri.clone();
    let doc = state.documents.get(&uri)?;
    let offset = position::to_offset(&doc.rope, params.text_document_position.position);
    let (_, _, token) = token_range(&doc.text, offset)?;
    if !is_valid_identifier(&params.new_name) { return None; }

    let edits: Vec<TextEdit> = references::find_word_occurrences(&doc.text, &token)
        .into_iter()
        .map(|(s, e)| TextEdit {
            range: Range {
                start: byte_to_position(&doc.rope, s),
                end:   byte_to_position(&doc.rope, e),
            },
            new_text: params.new_name.clone(),
        })
        .collect();
    if edits.is_empty() { return None; }

    let mut changes = HashMap::new();
    changes.insert(uri, edits);
    Some(WorkspaceEdit { changes: Some(changes), document_changes: None, change_annotations: None })
}

fn token_range(src: &str, offset: text_size::TextSize) -> Option<(usize, usize, String)> {
    let pos: usize = offset.into();
    let pos = pos.min(src.len());
    let bytes = src.as_bytes();
    let mut start = pos;
    while start > 0 && is_word(bytes[start - 1] as char) { start -= 1; }
    let mut end = pos;
    while end < bytes.len() && is_word(bytes[end] as char) { end += 1; }
    if start == end { return None; }
    Some((start, end, src[start..end].to_string()))
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }

fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
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
        if bytes_seen >= bytes_in_line { break; }
        utf16 += c.len_utf16() as u32;
        bytes_seen += c.len_utf8();
    }
    Position { line: line as u32, character: utf16 }
}
