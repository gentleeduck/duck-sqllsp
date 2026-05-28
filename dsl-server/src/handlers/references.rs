//! `textDocument/references` handler.
//!
//! Returns every occurrence of the identifier under the cursor across
//! every open buffer in the workspace. Strings, comments, and
//! dollar-quoted bodies are excluded so a literal `'orders'` does not
//! shadow a real reference. Identifier match is case-insensitive;
//! quoted identifiers are matched on their inner text without the
//! surrounding `"`. The cursor's own document is always included --
//! regardless of `include_declaration`, since SQL identifiers don't
//! have a single canonical "declaration" the way variables in
//! procedural languages do (think: a column appears in CREATE TABLE,
//! every SELECT that uses it, and in ALTER statements too).

use crate::handlers::position;
use crate::state::ServerState;
use ropey::Rope;
use tower_lsp::lsp_types::{Location, Position, Range, ReferenceParams};

pub fn run(state: &ServerState, params: ReferenceParams) -> Option<Vec<Location>> {
  let cursor_uri = params.text_document_position.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("references", &cursor_uri);
  let cursor_doc = state.documents.get(&cursor_uri)?;
  let offset = position::to_offset(&cursor_doc.rope, params.text_document_position.position);
  let token = token_at(&cursor_doc.text, offset)?;

  // Walk every open buffer, not just the cursor's, so refs follow
  // the schema across split-file migrations and seed scripts.
  let mut out = Vec::new();
  let mut seen_uris: std::collections::HashSet<tower_lsp::lsp_types::Url> = std::collections::HashSet::new();
  for (uri, doc) in state.documents.snapshot() {
    seen_uris.insert(uri.clone());
    for (start, end) in find_word_occurrences(&doc.text, &token) {
      out.push(Location {
        uri: uri.clone(),
        range: Range { start: byte_to_position(&doc.rope, start), end: byte_to_position(&doc.rope, end) },
      });
    }
  }
  // Disk fallback: walk every .sql in the workspace root that isn't
  // already an open buffer. Lets `Find All References` surface usages
  // in files the user hasn't opened yet.
  if let Some(root) = state.workspace_root.read().clone() {
    let mut count = 0usize;
    walk_sql_files(&root, 5000, &mut count, &mut |path| {
      let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(path) else { return };
      if seen_uris.contains(&uri) {
        return;
      }
      let Ok(text) = std::fs::read_to_string(path) else { return };
      let rope = Rope::from_str(&text);
      for (start, end) in find_word_occurrences(&text, &token) {
        out.push(Location {
          uri: uri.clone(),
          range: Range { start: byte_to_position(&rope, start), end: byte_to_position(&rope, end) },
        });
      }
    });
  }
  if out.is_empty() { None } else { Some(out) }
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

/// Walk `src` byte-by-byte, skip string/comment/dollar-quoted regions,
/// and yield (start, end) byte ranges where a whole-word case-insensitive
/// match of `needle` appears.
pub fn find_word_occurrences(src: &str, needle: &str) -> Vec<(usize, usize)> {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let needle_lower = needle.to_ascii_lowercase();
  let nlen = needle.len();
  let mut out = Vec::new();
  let mut i = 0usize;

  while i < n {
    let c = bytes[i] as char;

    // Line comment -- skip to end of line.
    if c == '-' && i + 1 < n && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
      continue;
    }
    // Block comment /* ... */ (PG nests these; we keep it simple).
    if c == '/' && i + 1 < n && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
      continue;
    }
    // Single-quoted string. Doubled '' is an escape; skip those too.
    if c == '\'' {
      i += 1;
      while i < n {
        if bytes[i] == b'\'' {
          if i + 1 < n && bytes[i + 1] == b'\'' {
            i += 2;
            continue;
          }
          i += 1;
          break;
        }
        i += 1;
      }
      continue;
    }
    // Dollar-quoted body: $$ ... $$ or $tag$ ... $tag$.
    if c == '$'
      && let Some((tag_end, close_match)) = dollar_open(bytes, i)
    {
      let mut j = tag_end;
      while j + close_match.len() <= n {
        if &bytes[j..j + close_match.len()] == close_match.as_bytes() {
          j += close_match.len();
          break;
        }
        j += 1;
      }
      i = j.min(n);
      continue;
    }
    // Double-quoted identifier "x" -- still searchable as the inner text.
    if c == '"' {
      let inner_start = i + 1;
      let mut j = inner_start;
      while j < n && bytes[j] != b'"' {
        j += 1;
      }
      let inner = &src[inner_start..j];
      if inner.eq_ignore_ascii_case(needle) {
        out.push((inner_start, j));
      }
      i = (j + 1).min(n);
      continue;
    }

    // Identifier candidate.
    if c.is_alphabetic() || c == '_' {
      let start = i;
      while i < n && is_word(bytes[i] as char) {
        i += 1;
      }
      if i - start == nlen && src[start..i].eq_ignore_ascii_case(&needle_lower) {
        out.push((start, i));
      }
      continue;
    }

    i += 1;
  }
  out
}

/// Parse a $$ or $tag$ opener at `i`. Returns (offset past opener, close
/// tag string) when valid, None when the `$` is something else (e.g. a
/// parameter placeholder `$1`).
fn dollar_open(bytes: &[u8], i: usize) -> Option<(usize, String)> {
  let n = bytes.len();
  if bytes[i] != b'$' {
    return None;
  }
  let mut j = i + 1;
  while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
    j += 1;
  }
  if j >= n || bytes[j] != b'$' {
    return None;
  }
  let tag = std::str::from_utf8(&bytes[i..=j]).ok()?.to_string();
  Some((j + 1, tag))
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
