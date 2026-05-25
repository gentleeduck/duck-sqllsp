//! `textDocument/hover` handler.

use crate::config::Case;
use crate::handlers::position;
use crate::state::ServerState;
use dsl_hover::{KeywordCase, hover_with};
use tower_lsp::lsp_types::{
  Hover, HoverContents, HoverParams, LanguageString, MarkedString, MarkupContent, MarkupKind,
};

pub fn run(state: &ServerState, params: HoverParams) -> Option<Hover> {
  let uri = params.text_document_position_params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("hover", &uri);
  let doc = state.documents.get(&uri)?;
  if doc.too_large() {
    return None;
  }
  let offset = position::to_offset(&doc.rope, params.text_document_position_params.position);
  // Offline-mode enrichment: merge live + buffer-derived catalogs so
  // hover finds sequences / types / extensions / roles defined only
  // in the buffer (no DB connection required).
  let live = state.catalog.read().clone();
  let cache = doc.parsed();
  let derived = dsl_completion::source_tables::from_source(&cache.file, &doc.text);
  let ws_offline = state.workspace_offline_snapshot();
  let cat = dsl_completion::source_tables::merge(
    &dsl_completion::source_tables::merge(&live, &derived),
    &ws_offline,
  );
  let case = match state.config_snapshot().style.keyword {
    Case::Upper => KeywordCase::Upper,
    Case::Lower => KeywordCase::Lower,
    Case::Preserve => KeywordCase::Preserve,
  };
  let md = hover_with(&doc.text, offset, &cat, case)?;
  let range = hover_range_for(&doc.text, &doc.rope, offset);
  Some(Hover { contents: split_markdown_fences(&md), range })
}

/// Compute the hover-target range so the editor highlights the
/// whole token / literal under the cursor. Without this the client
/// underlines only the cursor's position.
fn hover_range_for(source: &str, rope: &ropey::Rope, offset: text_size::TextSize) -> Option<tower_lsp::lsp_types::Range> {
  let pos: usize = u32::from(offset) as usize;
  let bytes = source.as_bytes();
  if pos > bytes.len() { return None; }
  // String literal: walk back to opening `'`, forward to closing `'`.
  if let Some(span) = enclosing_string(bytes, pos) {
    return Some(tower_lsp::lsp_types::Range {
      start: position::byte_to_lsp(rope, span.0),
      end: position::byte_to_lsp(rope, span.1),
    });
  }
  // Identifier / number: word-bounded.
  let mut s = pos.min(bytes.len());
  while s > 0 && is_word(bytes[s - 1]) {
    s -= 1;
  }
  let mut e = pos.min(bytes.len());
  while e < bytes.len() && is_word(bytes[e]) {
    e += 1;
  }
  if s == e { return None; }
  Some(tower_lsp::lsp_types::Range {
    start: position::byte_to_lsp(rope, s),
    end: position::byte_to_lsp(rope, e),
  })
}

fn is_word(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' }

/// Span of the single-quoted string literal containing `pos` (inclusive
/// of both quotes). None when pos isn't inside one.
fn enclosing_string(bytes: &[u8], pos: usize) -> Option<(usize, usize)> {
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    if bytes[i] == b'\'' {
      let start = i;
      i += 1;
      while i < n {
        if bytes[i] == b'\'' {
          if i + 1 < n && bytes[i + 1] == b'\'' { i += 2; continue; }
          let end = i + 1;
          if pos >= start && pos <= end { return Some((start, end)); }
          i = end;
          break;
        }
        i += 1;
      }
      continue;
    }
    i += 1;
  }
  None
}

/// Split the hover markdown at every ```sql ... ``` fence and return a
/// `MarkedString[]` mixing markdown chunks with language-tagged code
/// strings.
///
/// Why: nvim's stock hover handler applies vim's `sql.vim` syntax to
/// every `LanguageString { language: "sql", ... }` natively -- no
/// tree-sitter `sql` parser required. With the previous Markdown-only
/// hover, the SQL inside the fence relied on a markdown-to-sql
/// tree-sitter injection that not every client has set up; the result
/// was the whole card painting as one italic blob.
fn split_markdown_fences(md: &str) -> HoverContents {
  let mut chunks: Vec<MarkedString> = Vec::new();
  let mut rest = md;
  while let Some(open_at) = rest.find("```sql") {
    let pre = &rest[..open_at];
    if !pre.trim().is_empty() {
      chunks.push(MarkedString::String(pre.trim_end_matches('\n').to_string()));
    }
    let after_open = &rest[open_at + "```sql".len()..];
    let after_open = after_open.trim_start_matches('\n');
    let Some(close_rel) = after_open.find("```") else {
      // Unterminated fence -- bail to the original markdown.
      chunks.push(MarkedString::String(rest.to_string()));
      return wrap(chunks);
    };
    let code = &after_open[..close_rel];
    chunks.push(MarkedString::LanguageString(LanguageString {
      language: "sql".into(),
      value: code.trim_end_matches('\n').to_string(),
    }));
    rest = &after_open[close_rel + 3..];
  }
  if !rest.trim().is_empty() {
    chunks.push(MarkedString::String(rest.trim_start_matches('\n').to_string()));
  }
  wrap(chunks)
}

fn wrap(chunks: Vec<MarkedString>) -> HoverContents {
  if chunks.is_empty() {
    return HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: String::new() });
  }
  if chunks.len() == 1 {
    if let Some(MarkedString::String(s)) = chunks.first() {
      return HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: s.clone() });
    }
  }
  HoverContents::Array(chunks)
}
