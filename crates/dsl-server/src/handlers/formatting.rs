//! `textDocument/formatting` handler.
//!
//! Thin LSP shim: read the open document, hand the text to
//! [`dsl_format::format`], wrap the result in a whole-document TextEdit.
//! All real work lives in `dsl-format`.

use crate::state::ServerState;
use tower_lsp::lsp_types::{DocumentFormattingParams, Position, Range, TextEdit};

pub fn run(state: &ServerState, params: DocumentFormattingParams) -> Option<Vec<TextEdit>> {
    let uri = &params.text_document.uri;
    let doc = state.documents.get(uri)?;
    let original = doc.text.clone();
    let cfg = state.config_snapshot();

    let formatted = dsl_format::format(&original, &cfg.style.formatter, &cfg.style.create_table);
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
