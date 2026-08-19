//! `textDocument/formatting` handler.
//!
//! Thin LSP shim: read the open document, hand the text to
//! [`dsl_format::format`], wrap the result in a whole-document TextEdit.
//! All real work lives in `dsl-format`.

use crate::documents::Document;
use crate::state::ServerState;
use tower_lsp::lsp_types::{DocumentFormattingParams, FormattingOptions, Position, Range, TextEdit};

/// Resolve the effective [`dsl_format::FormatterStyle`] for one format
/// request: global config, overlaid with the editor-sent
/// `FormattingOptions`, then dialect-adjusted.
///
/// Shared by `textDocument/formatting` and
/// `textDocument/rangeFormatting` so both honor the same knobs.
pub(crate) fn resolve_style(
  state: &ServerState,
  doc: &Document,
  options: &FormattingOptions,
) -> dsl_format::FormatterStyle {
  // Honor the LSP-standard FormattingOptions the editor sent. tab_size
  // overrides the formatter's tabWidth (per-buffer wins over global
  // config) so the editor's `:set tabstop=2` is respected for this one
  // format request. insert_spaces is informational for now; sql-
  // formatter always emits spaces. trim_trailing_whitespace and
  // insert_final_newline are normalised already by the post-pass.
  let mut style = state.config_snapshot().style.formatter.clone();
  if options.tab_size > 0 {
    style.tab_width = options.tab_size as usize;
  }

  // Dialect-aware formatter language. When the user hasn't pinned
  // `formatter.language` away from the default (postgresql), let the
  // open buffer's dialect drive sql-formatter's `-l` flag so it
  // tokenises `\`backticks\`` (mysql) or `[brackets]` (mssql) instead
  // of treating them as garbage.
  if style.language == "postgresql" {
    style.language = match doc.dialect {
      dsl_parse::Dialect::Postgres => "postgresql".into(),
      dsl_parse::Dialect::MySql => "mysql".into(),
      dsl_parse::Dialect::SQLite => "sqlite".into(),
      dsl_parse::Dialect::MsSql => "transactsql".into(),
      dsl_parse::Dialect::Generic => "sql".into(),
    };
  }
  style
}

pub fn run(state: &ServerState, params: DocumentFormattingParams) -> Option<Vec<TextEdit>> {
  let uri = &params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("formatting", uri);
  let doc = state.documents.get(uri)?;
  let original = doc.text.clone();
  let cfg = state.config_snapshot();
  let formatter_style = resolve_style(state, &doc, &params.options);

  // Format cache: hash (input + style key) to skip the sql-formatter +
  // align pipeline when nothing changed since the last call. The hash
  // covers the raw text plus the single bit of formatter style most
  // likely to change at runtime (singleLine), so toggling that knob
  // still invalidates the entry.
  let key = uri.to_string();
  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  use std::hash::{Hash, Hasher};
  original.hash(&mut hasher);
  formatter_style.single_line.hash(&mut hasher);
  formatter_style.compact_clauses.hash(&mut hasher);
  formatter_style.tab_width.hash(&mut hasher);
  formatter_style.language.hash(&mut hasher);
  let input_hash = hasher.finish();
  if let Some((cached_hash, cached_out)) = state.format_cache.read().get(&key)
    && *cached_hash == input_hash
  {
    if cached_out == &original {
      return None;
    }
    let rope = &doc.rope;
    let last_line = rope.len_lines().saturating_sub(1) as u32;
    let last_line_text = rope.line(last_line as usize);
    let last_col = last_line_text.chars().filter(|c| *c != '\n' && *c != '\r').count() as u32;
    return Some(vec![TextEdit {
      range: Range {
        start: Position { line: 0, character: 0 },
        end: Position { line: last_line, character: last_col },
      },
      new_text: cached_out.clone(),
    }]);
  }

  let formatted = dsl_format::format(&original, &formatter_style, &cfg.style.create_table);
  state.format_cache.write().insert(key, (input_hash, formatted.clone()));
  if formatted == original {
    return None;
  }

  // End-of-document position: nvim 0.11 silently drops TextEdits whose end
  // range extends past the buffer's line count. Use the rope's last line
  // index + the column count at that line so the range covers exactly the
  // document and not one past it.
  let rope = &doc.rope;
  let last_line = rope.len_lines().saturating_sub(1) as u32;
  let last_line_text = rope.line(last_line as usize);
  let last_col = last_line_text.chars().filter(|c| *c != '\n' && *c != '\r').count() as u32;
  Some(vec![TextEdit {
    range: Range { start: Position { line: 0, character: 0 }, end: Position { line: last_line, character: last_col } },
    new_text: formatted,
  }])
}
