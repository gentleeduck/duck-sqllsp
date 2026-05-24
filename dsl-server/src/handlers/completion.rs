//! `textDocument/completion` handler.

use crate::config::{Case, Style};
use crate::handlers::position;
use crate::state::ServerState;
use dsl_completion::{complete as engine_complete, Item, ItemKind};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, CompletionParams,
    CompletionResponse, Documentation, InsertTextFormat, MarkupContent, MarkupKind,
};

pub fn run(state: &ServerState, params: CompletionParams) -> Option<CompletionResponse> {
    let uri = params.text_document_position.text_document.uri;
    let doc = state.documents.get(&uri)?;
    if doc.too_large() { return None; }
    let offset = position::to_offset(&doc.rope, params.text_document_position.position);
    let cache = doc.parsed();
    let cat = state.catalog.read();
    let items = engine_complete(&doc.text, &cache.file, &cache.scopes, &*cat, offset);

    let style = state.config_snapshot().style;
    let lsp_items: Vec<CompletionItem> = items
        .into_iter()
        .map(|it| to_lsp_item(it, &style))
        .collect();

    tracing::debug!(uri = %uri, count = lsp_items.len(), "completion");
    Some(CompletionResponse::Array(lsp_items))
}

fn case_for(kind: ItemKind, style: &Style) -> Case {
    match kind {
        ItemKind::Keyword => style.keyword,
        ItemKind::Function => style.function,
        ItemKind::Type => style.type_,
        // Tables / columns / schemas / views are identifiers from the
        // database; respect the user's existing casing by default.
        ItemKind::Table | ItemKind::View | ItemKind::Column | ItemKind::Schema => style.identifier,
        // PL/pgSQL locals and parameters are user-named -- never recased.
        ItemKind::Variable | ItemKind::Parameter => Case::Preserve,
    }
}

fn to_lsp_item(it: Item, style: &Style) -> CompletionItem {
    let case = case_for(it.kind, style);
    let label = case.apply(&it.label);
    // Snippet insert text must keep `$0` / `${n:label}` placeholders
    // verbatim; only re-case the function-name prefix before the `(`.
    let insert = if it.is_snippet {
        if let Some(paren) = it.insert_text.find('(') {
            let head = case.apply(&it.insert_text[..paren]);
            format!("{head}{}", &it.insert_text[paren..])
        } else {
            it.insert_text.clone()
        }
    } else {
        case.apply(&it.insert_text)
    };
    // `sortText` is a string; clients compare lexicographically. Use a
    // single ASCII digit prefix (0..9) so lower `sort_priority` wins.
    // The label tail breaks ties alphabetically within the same prio.
    let sort_text = format!("{}{}", it.sort_priority.min(9), &label);
    CompletionItem {
        label,
        kind: Some(kind(it.kind)),
        detail: it.detail,
        label_details: it.description.map(|d| CompletionItemLabelDetails {
            detail: None,
            description: Some(d),
        }),
        documentation: it.documentation_md.map(|md| {
            Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: md,
            })
        }),
        insert_text: Some(insert),
        insert_text_format: if it.is_snippet {
            Some(InsertTextFormat::SNIPPET)
        } else {
            None
        },
        sort_text: Some(sort_text),
        ..Default::default()
    }
}

fn kind(k: ItemKind) -> CompletionItemKind {
    match k {
        ItemKind::Keyword => CompletionItemKind::KEYWORD,
        ItemKind::Type => CompletionItemKind::TYPE_PARAMETER,
        ItemKind::Function => CompletionItemKind::FUNCTION,
        ItemKind::Table => CompletionItemKind::CLASS,
        ItemKind::View => CompletionItemKind::INTERFACE,
        ItemKind::Column => CompletionItemKind::FIELD,
        ItemKind::Schema => CompletionItemKind::MODULE,
        ItemKind::Variable => CompletionItemKind::VARIABLE,
        ItemKind::Parameter => CompletionItemKind::VARIABLE,
    }
}
