//! `textDocument/hover` handler.

use crate::config::Case;
use crate::handlers::position;
use crate::state::ServerState;
use dsl_hover::{hover_with, KeywordCase};
use tower_lsp::lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind};

pub fn run(state: &ServerState, params: HoverParams) -> Option<Hover> {
    let uri = params.text_document_position_params.text_document.uri;
    let doc = state.documents.get(&uri)?;
    if doc.too_large() { return None; }
    let offset = position::to_offset(&doc.rope, params.text_document_position_params.position);
    let cat = state.catalog.read();
    let case = match state.config_snapshot().style.keyword {
        Case::Upper    => KeywordCase::Upper,
        Case::Lower    => KeywordCase::Lower,
        Case::Preserve => KeywordCase::Preserve,
    };
    let md = hover_with(&doc.text, offset, &*cat, case)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: md,
        }),
        range: None,
    })
}
