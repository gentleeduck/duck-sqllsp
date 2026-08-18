//! `completionItem/resolve` handler.
//!
//! Completion is the hottest request in the server -- it fires on nearly
//! every keystroke -- and the expensive part was never the candidate
//! search. It was documentation: ~2700 built-in keywords, types, and
//! functions, each rendered to markdown (several `format!`s plus
//! paragraph wrapping) and serialised into the response, so the client
//! could display exactly one of them.
//!
//! `completionItem/resolve` is the LSP answer. The list response now
//! ships items without `documentation`, plus a small `data` blob naming
//! what the item was. When the user actually highlights an entry, the
//! client sends it back here and we render the docs for that one item.
//!
//! The `data` blob is deliberately tiny and self-describing:
//!
//! ```json
//! { "l": "count", "k": 2, "s": "count($0)" }
//! ```
//!
//! `l` is the *pre-casing* label (the list response may have upper- or
//! lower-cased it per the user's style config), `k` is the [`ItemKind`]
//! discriminant, and `s` is the snippet insert text when the item is a
//! snippet. Everything needed to rebuild the documentation, nothing
//! that would have cost bandwidth to ship in the first place.
//!
//! Items whose docs are catalog-derived (tables, columns, DB functions)
//! keep their markdown inline in the list response: it is already built
//! by the engine and there is no cheaper key to send instead. Deferring
//! those too would mean re-resolving them against the catalog here, and
//! is the obvious next step if large-schema completion ever shows up in
//! a profile.
//!
//! Resolve must be total: an unknown, absent, or malformed `data` blob
//! returns the item untouched rather than erroring. Clients cache
//! resolved items across edits and will happily send back something we
//! no longer recognise.

use dsl_completion::ItemKind;
use tower_lsp::lsp_types::{CompletionItem, Documentation, MarkupContent, MarkupKind};

/// Compact `CompletionItem::data` payload. Field names are one letter
/// because this round-trips through the client for every item in every
/// completion response.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ResolveData {
  /// Label before the user's case style was applied.
  pub l: String,
  /// [`ItemKind`] discriminant.
  pub k: u8,
  /// Snippet insert text, when `is_snippet`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub s: Option<String>,
  /// Hand-written blurb for snippet items, which has no lookup key to
  /// rebuild it from. Carrying it here costs the same bytes as shipping
  /// it as `documentation` would have, and only the ~30 scaffold
  /// snippets have one -- but it lets resolve compose the blurb
  /// *underneath* the expansion preview instead of the client showing a
  /// blurb with no preview.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub d: Option<String>,
}

pub fn kind_code(k: ItemKind) -> u8 {
  match k {
    ItemKind::Keyword => 0,
    ItemKind::Type => 1,
    ItemKind::Function => 2,
    ItemKind::Table => 3,
    ItemKind::View => 4,
    ItemKind::Column => 5,
    ItemKind::Schema => 6,
    ItemKind::Variable => 7,
    ItemKind::Parameter => 8,
  }
}

pub fn run(mut item: CompletionItem) -> CompletionItem {
  let _g = crate::handlers::perf::Guard::new("completion_resolve");

  // Already resolved (or never needed resolving) -- don't rebuild.
  if item.documentation.is_some() {
    return item;
  }
  let Some(data) = item.data.clone() else { return item };
  let Ok(data) = serde_json::from_value::<ResolveData>(data) else { return item };

  // A hand-written blurb wins over the knowledge base -- it was written
  // for this specific item.
  let kb_md = data.d.clone().or_else(|| lookup_markdown(&data.l, data.k));
  let value = match data.s.as_deref() {
    Some(snippet) => {
      // Snippet items lead with the rendered expansion, then any
      // knowledge-base prose underneath -- same layout the eager path
      // produced before resolve existed.
      let preview = crate::handlers::completion::render_snippet_preview(snippet);
      let header = format!("**Expands to:**\n\n```sql\n{preview}\n```\n");
      match kb_md {
        Some(md) if !md.trim().is_empty() => format!("{header}\n---\n\n{md}"),
        _ => header,
      }
    },
    None => match kb_md {
      Some(md) => md,
      None => return item,
    },
  };

  item.documentation = Some(Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value }));
  item
}

/// Re-find the knowledge-base entry for a label. Keyword and type
/// tables key on uppercase, functions on lowercase, so the lookup is
/// case-insensitive and survives whatever casing the style config
/// applied to the label we sent out.
fn lookup_markdown(label: &str, kind: u8) -> Option<String> {
  let entry = match kind {
    0 => dsl_knowledge::keywords().get(label.to_ascii_uppercase().as_str()),
    1 => dsl_knowledge::types().get(label.to_ascii_uppercase().as_str()),
    2 => dsl_knowledge::functions().get(label.to_ascii_lowercase().as_str()),
    _ => None,
  }?;
  Some(dsl_knowledge::render_markdown(entry))
}

#[cfg(test)]
mod tests {
  use super::*;
  use tower_lsp::lsp_types::CompletionItem;

  fn item_with(data: ResolveData) -> CompletionItem {
    CompletionItem { label: data.l.clone(), data: Some(serde_json::to_value(&data).unwrap()), ..Default::default() }
  }

  fn doc_text(item: &CompletionItem) -> String {
    match item.documentation.as_ref() {
      Some(Documentation::MarkupContent(m)) => m.value.clone(),
      other => panic!("expected markdown documentation, got {other:?}"),
    }
  }

  #[test]
  fn resolves_keyword_documentation() {
    let out = run(item_with(ResolveData { l: "SELECT".into(), k: 0, s: None, d: None }));
    assert!(doc_text(&out).contains("SELECT"));
  }

  #[test]
  fn resolves_function_documentation_case_insensitively() {
    // The style config may have upper-cased the label on the way out.
    let out = run(item_with(ResolveData { l: "COUNT".into(), k: 2, s: None, d: None }));
    assert!(doc_text(&out).to_ascii_lowercase().contains("count"));
  }

  #[test]
  fn snippet_items_lead_with_the_expansion_preview() {
    let out = run(item_with(ResolveData {
      l: "ctable".into(),
      k: 0,
      s: Some("CREATE TABLE ${1:name} (\n  id int\n);$0".into()),
      d: None,
    }));
    let doc = doc_text(&out);
    assert!(doc.starts_with("**Expands to:**"), "got {doc:?}");
    assert!(doc.contains("CREATE TABLE name"), "placeholders should render as their default text: {doc:?}");
    assert!(!doc.contains("${1:"), "raw placeholders must not leak: {doc:?}");
  }

  #[test]
  fn unknown_label_is_returned_untouched() {
    let out = run(item_with(ResolveData { l: "not_a_real_builtin_xyz".into(), k: 2, s: None, d: None }));
    assert!(out.documentation.is_none());
  }

  #[test]
  fn missing_data_is_returned_untouched() {
    let out = run(CompletionItem { label: "SELECT".into(), ..Default::default() });
    assert!(out.documentation.is_none());
  }

  #[test]
  fn malformed_data_is_returned_untouched() {
    let out = run(CompletionItem {
      label: "SELECT".into(),
      data: Some(serde_json::json!({ "unexpected": true })),
      ..Default::default()
    });
    assert!(out.documentation.is_none());
  }

  #[test]
  fn already_documented_items_are_not_rebuilt() {
    let mut item = item_with(ResolveData { l: "SELECT".into(), k: 0, s: None, d: None });
    item.documentation =
      Some(Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value: "kept".into() }));
    assert_eq!(doc_text(&run(item)), "kept");
  }

  #[test]
  fn kind_codes_round_trip_through_every_variant() {
    // A code collision would resolve an item against the wrong table.
    let all = [
      ItemKind::Keyword,
      ItemKind::Type,
      ItemKind::Function,
      ItemKind::Table,
      ItemKind::View,
      ItemKind::Column,
      ItemKind::Schema,
      ItemKind::Variable,
      ItemKind::Parameter,
    ];
    let mut codes: Vec<u8> = all.iter().map(|k| kind_code(*k)).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), all.len(), "kind codes must be unique");
  }
}
