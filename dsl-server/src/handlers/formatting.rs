//! `textDocument/formatting` handler.
//!
//! Thin LSP shim: read the open document, hand the text to
//! [`dsl_format::format`], wrap the result in a whole-document TextEdit.
//! All real work lives in `dsl-format`.

use crate::state::ServerState;
use tower_lsp::lsp_types::{DocumentFormattingParams, Position, Range, TextEdit};

pub fn run(state: &ServerState, params: DocumentFormattingParams) -> Option<Vec<TextEdit>> {
    let uri = &params.text_document.uri;
    let _g = crate::handlers::perf::Guard::with_uri("formatting", uri);
    let doc = state.documents.get(uri)?;
    let original = doc.text.clone();
    let cfg = state.config_snapshot();

    // Honor the LSP-standard FormattingOptions the editor sent. tab_size
    // overrides the formatter's tabWidth (per-buffer wins over global
    // config) so the editor's `:set tabstop=2` is respected for this one
    // format request. insert_spaces is informational for now; sql-
    // formatter always emits spaces. trim_trailing_whitespace and
    // insert_final_newline are normalised already by the post-pass.
    let mut formatter_style = cfg.style.formatter.clone();
    if params.options.tab_size > 0 {
        formatter_style.tab_width = params.options.tab_size as usize;
    }

    let formatted = dsl_format::format(&original, &formatter_style, &cfg.style.create_table);
    if formatted == original { return None; }

    let last_line_idx = original.lines().count() as u32;
    Some(vec![TextEdit {
        range: Range {
            start: Position { line: 0, character: 0 },
            end:   Position { line: last_line_idx + 1, character: 0 },
        },
        new_text: formatted,
    }])
}
