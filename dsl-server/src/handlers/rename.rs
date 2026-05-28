//! `textDocument/rename` + `textDocument/prepareRename`.
//!
//! Workspace-scoped rename: every whole-word occurrence of the
//! identifier under the cursor is rewritten across every open buffer.
//! Strings, comments, and dollar-quoted bodies are skipped (see
//! `references::find_word_occurrences`). The cursor's document is
//! always included.

use crate::handlers::{position, references};
use crate::state::ServerState;
use ropey::Rope;
use std::collections::HashMap;
use tower_lsp::lsp_types::{
  Position, PrepareRenameResponse, Range, RenameParams, TextDocumentPositionParams, TextEdit, WorkspaceEdit,
};

pub fn prepare(state: &ServerState, params: TextDocumentPositionParams) -> Option<PrepareRenameResponse> {
  let doc = state.documents.get(&params.text_document.uri)?;
  let offset = position::to_offset(&doc.rope, params.position);
  let (start, end, token) = token_range(&doc.text, offset)?;
  Some(PrepareRenameResponse::RangeWithPlaceholder {
    range: Range { start: byte_to_position(&doc.rope, start), end: byte_to_position(&doc.rope, end) },
    placeholder: token,
  })
}

pub fn run(state: &ServerState, params: RenameParams) -> Option<WorkspaceEdit> {
  let cursor_uri = params.text_document_position.text_document.uri.clone();
  let _g = crate::handlers::perf::Guard::with_uri("rename", &cursor_uri);
  let cursor_doc = state.documents.get(&cursor_uri)?;
  let offset = position::to_offset(&cursor_doc.rope, params.text_document_position.position);
  let (_, _, token) = token_range(&cursor_doc.text, offset)?;
  if !is_valid_identifier(&params.new_name) {
    return None;
  }

  // Walk every open buffer so a rename across split-file migrations
  // /seed scripts updates them in one operation.
  let mut changes: HashMap<_, Vec<TextEdit>> = HashMap::new();
  let mut seen: std::collections::HashSet<tower_lsp::lsp_types::Url> = std::collections::HashSet::new();
  for (uri, doc) in state.documents.snapshot() {
    seen.insert(uri.clone());
    let edits: Vec<TextEdit> = references::find_word_occurrences(&doc.text, &token)
      .into_iter()
      .map(|(s, e)| TextEdit {
        range: Range { start: byte_to_position(&doc.rope, s), end: byte_to_position(&doc.rope, e) },
        new_text: params.new_name.clone(),
      })
      .collect();
    if !edits.is_empty() {
      changes.insert(uri, edits);
    }
  }
  // Disk fallback: walk every workspace .sql that isn't open and
  // rewrite there too. Editor will apply the TextEdits via
  // WorkspaceEdit.changes on the closed files; client opens / edits /
  // saves them transparently.
  if let Some(root) = state.workspace_root.read().clone() {
    let mut count = 0usize;
    walk_sql_files(&root, 5000, &mut count, &mut |path| {
      let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(path) else { return };
      if seen.contains(&uri) {
        return;
      }
      let Ok(text) = std::fs::read_to_string(path) else { return };
      let rope = Rope::from_str(&text);
      let edits: Vec<TextEdit> = references::find_word_occurrences(&text, &token)
        .into_iter()
        .map(|(s, e)| TextEdit {
          range: Range { start: byte_to_position(&rope, s), end: byte_to_position(&rope, e) },
          new_text: params.new_name.clone(),
        })
        .collect();
      if !edits.is_empty() {
        changes.insert(uri, edits);
      }
    });
  }
  if changes.is_empty() {
    return None;
  }
  Some(WorkspaceEdit { changes: Some(changes), document_changes: None, change_annotations: None })
}

fn walk_sql_files(root: &std::path::Path, cap: usize, count: &mut usize, f: &mut impl FnMut(&std::path::Path)) {
  if *count >= cap {
    return;
  }
  let Ok(rd) = std::fs::read_dir(root) else { return };
  for entry in rd.flatten() {
    if *count >= cap {
      return;
    }
    let path = entry.path();
    if let Some(name) = path.file_name().and_then(|s| s.to_str())
      && (name.starts_with('.') || matches!(name, "node_modules" | "target" | "dist" | "build" | "vendor" | "out"))
    {
      continue;
    }
    if path.is_dir() {
      walk_sql_files(&path, cap, count, f);
    } else if let Some(ext) = path.extension().and_then(|s| s.to_str())
      && matches!(ext.to_ascii_lowercase().as_str(), "sql" | "pgsql" | "psql")
    {
      *count += 1;
      f(&path);
    }
  }
}

fn token_range(src: &str, offset: text_size::TextSize) -> Option<(usize, usize, String)> {
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
  Some((start, end, src[start..end].to_string()))
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn is_valid_identifier(s: &str) -> bool {
  let mut chars = s.chars();
  match chars.next() {
    Some(c) if c.is_alphabetic() || c == '_' => {},
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
    if bytes_seen >= bytes_in_line {
      break;
    }
    utf16 += c.len_utf16() as u32;
    bytes_seen += c.len_utf8();
  }
  Position { line: line as u32, character: utf16 }
}
