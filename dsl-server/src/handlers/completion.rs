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

/// Render an LSP snippet string as a previewable SQL fragment.
///
/// Recognises:
///   * `${n:label}`            -> `label`              (uses default text)
///   * `${n|a,b,c|}`           -> `a`                  (uses first choice)
///   * `${n}`                  -> ``                   (drops placeholder)
///   * `$0` / `$1` ...         -> ``                   (drops tabstop)
///
/// Brace and pipe matching is depth-aware so a choice with nested
/// placeholders (`${1|TYPE ${2:type},SET DEFAULT ${3:expr}|}`) is
/// parsed as one outer placeholder rather than tripping over the inner
/// `}`. Recurses on the chosen text so the inner placeholders also get
/// rendered.
pub(crate) fn render_snippet_preview(snippet: &str) -> String {
    let bytes = snippet.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        if bytes[i] == b'$' && i + 1 < n {
            if bytes[i + 1] == b'{' {
                // Balanced match: walk forward counting `{` and `}`,
                // skipping past nested `${...}` bodies.
                let body_start = i + 2;
                let mut depth = 1i32;
                let mut j = body_start;
                while j < n && depth > 0 {
                    match bytes[j] {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 { break; }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                if j < n {
                    let body = &snippet[body_start..j];
                    // Split off the leading `n` (digits) -- doesn't matter
                    // for rendering; the choice payload starts at the
                    // first `:` or `|` that is not inside a nested `${}`.
                    let payload_start = body
                        .as_bytes()
                        .iter()
                        .position(|&b| !b.is_ascii_digit())
                        .unwrap_or(body.len());
                    let payload = &body[payload_start..];
                    let chosen: String = if let Some(rest) = payload.strip_prefix(':') {
                        rest.into()
                    } else if let Some(rest) = payload.strip_prefix('|') {
                        // `|a,b,c|` -- pick the first top-level alternative.
                        let mut depth2 = 0i32;
                        let mut end = 0usize;
                        let bs = rest.as_bytes();
                        for (k, &b) in bs.iter().enumerate() {
                            match b {
                                b'{' => depth2 += 1,
                                b'}' => depth2 -= 1,
                                b',' if depth2 == 0 => { end = k; break; }
                                b'|' if depth2 == 0 => { end = k; break; }
                                _ => {}
                            }
                            if k == bs.len() - 1 { end = k + 1; }
                        }
                        rest[..end].into()
                    } else {
                        String::new()
                    };
                    out.push_str(&render_snippet_preview(&chosen));
                    i = j + 1;
                    continue;
                }
            }
            // Bare `$0` / `$1` -- skip the digit(s).
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

#[cfg(test)]
mod tests {
    use super::render_snippet_preview;

    #[test]
    fn renders_default_text_placeholder() {
        assert_eq!(render_snippet_preview("SELECT ${1:col} FROM ${2:tbl};"),
                   "SELECT col FROM tbl;");
    }

    #[test]
    fn renders_first_choice_from_simple_chooser() {
        assert_eq!(render_snippet_preview("${1|ASC,DESC|}"), "ASC");
    }

    #[test]
    fn renders_nested_choices_with_inner_placeholders() {
        // Real ALTER COLUMN snippet shipped in dsl-completion::sources.
        let snippet = "ALTER COLUMN ${1:name} ${2|TYPE ${3:type},SET DEFAULT ${4:expr},DROP DEFAULT|}";
        let preview = render_snippet_preview(snippet);
        assert_eq!(preview, "ALTER COLUMN name TYPE type");
    }

    #[test]
    fn strips_bare_tabstops() {
        assert_eq!(render_snippet_preview("SELECT 1$0;"), "SELECT 1;");
    }

    #[test]
    fn empty_placeholder_drops_with_no_text() {
        assert_eq!(render_snippet_preview("a ${1} b"), "a  b");
    }
}
