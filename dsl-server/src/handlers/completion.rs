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
    let _g = crate::handlers::perf::Guard::with_uri("completion", &uri);
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
    // For snippet items, prepend an "Expands to:" preview to the
    // documentation so users can see the final scaffold (with $1 / $0
    // placeholders cleaned out) before they pick it.
    let documentation = build_documentation(&it.documentation_md, it.is_snippet, &it.insert_text);
    CompletionItem {
        label,
        kind: Some(kind(it.kind)),
        detail: it.detail,
        label_details: it.description.map(|d| CompletionItemLabelDetails {
            detail: None,
            description: Some(d),
        }),
        documentation,
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

fn build_documentation(
    md: &Option<String>,
    is_snippet: bool,
    insert_text: &str,
) -> Option<Documentation> {
    if !is_snippet {
        return md.as_ref().map(|m| Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: m.clone(),
        }));
    }
    // Replace `${n:placeholder}` -> `placeholder`,
    // `${n|a,b,c|}` -> `a`, and bare `$n` / `$0` -> ``.
    let preview = render_snippet_preview(insert_text);
    let header = format!("**Expands to:**\n\n```sql\n{preview}\n```\n");
    let value = match md {
        Some(m) if !m.trim().is_empty() => format!("{header}\n---\n\n{m}"),
        _ => header,
    };
    Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value,
    }))
}

fn render_snippet_preview(snippet: &str) -> String {
    let bytes = snippet.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        if bytes[i] == b'$' && i + 1 < n {
            // `${...}` form
            if bytes[i + 1] == b'{' {
                // Find matching `}`.
                let mut j = i + 2;
                while j < n && bytes[j] != b'}' { j += 1; }
                if j < n {
                    let body = &snippet[i + 2..j];
                    // `n:label` -> label. `n|a,b,c|` -> a. `n` -> ``.
                    if let Some(colon) = body.find(':') {
                        out.push_str(&body[colon + 1..]);
                    } else if let Some(pipe_open) = body.find('|') {
                        let inside = &body[pipe_open + 1..];
                        let inside = inside.trim_end_matches('|');
                        let first = inside.split(',').next().unwrap_or("");
                        out.push_str(first);
                    }
                    i = j + 1;
                    continue;
                }
            }
            // Bare `$0` / `$1` etc — skip the digit(s).
            let mut j = i + 1;
            while j < n && bytes[j].is_ascii_digit() { j += 1; }
            i = j;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
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
