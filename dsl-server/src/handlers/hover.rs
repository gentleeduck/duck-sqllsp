//! `textDocument/hover` handler.

use crate::config::Case;
use crate::handlers::position;
use crate::state::ServerState;
use dsl_hover::{hover_with, KeywordCase};
use tower_lsp::lsp_types::{
    Hover, HoverContents, HoverParams, LanguageString, MarkedString, MarkupContent, MarkupKind,
};

pub fn run(state: &ServerState, params: HoverParams) -> Option<Hover> {
    let uri = params.text_document_position_params.text_document.uri;
    let _g = crate::handlers::perf::Guard::with_uri("hover", &uri);
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
        contents: split_markdown_fences(&md),
        range: None,
    })
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
        return HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: String::new(),
        });
    }
    if chunks.len() == 1 {
        if let Some(MarkedString::String(s)) = chunks.first() {
            return HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: s.clone(),
            });
        }
    }
    HoverContents::Array(chunks)
}
