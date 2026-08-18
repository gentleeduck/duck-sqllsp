//! Crate-local representation of one completion entry.
//!
//! Maps cleanly onto LSP `CompletionItem` in the server, but lives here
//! so analysis/testing/inspection don't have to pull in tower-lsp.

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Item {
  pub label: String,
  pub kind: ItemKind,
  pub detail: Option<String>,
  pub description: Option<String>,
  /// Eagerly-built markdown, for items whose documentation is derived
  /// from the catalog or a hand-written snippet blurb. Knowledge-base
  /// items leave this `None` and set [`Item::kb_entry`] instead -- read
  /// through [`Item::documentation`] rather than touching either field.
  pub documentation_md: Option<String>,
  /// Knowledge-base entry backing this item, when it has one.
  ///
  /// Rendering markdown for every built-in keyword, type, and function
  /// costs ~2700 `render_markdown` calls (each one several `format!`s
  /// plus paragraph wrapping) on *every* completion request, and the
  /// client displays exactly one of them. Carrying the entry instead
  /// lets the server defer rendering to `completionItem/resolve`.
  #[serde(skip)]
  pub kb_entry: Option<&'static dsl_knowledge::Entry>,
  pub insert_text: String,
  /// `true` when `insert_text` contains LSP snippet placeholders like
  /// `$0` or `${1:arg}`. The server maps this onto
  /// `InsertTextFormat::Snippet`. Default `false`.
  #[serde(default)]
  pub is_snippet: bool,
  /// Sort priority -- lower = appears first in the completion menu.
  /// 0 = in-scope columns (from FROM/JOIN), 1 = in-scope tables,
  /// 2 = scoped builtins, 3 = catalog-wide tables/functions,
  /// 4 = keywords, 5 = catch-all. Default 5 keeps old call-sites safe.
  #[serde(default = "default_sort")]
  pub sort_priority: u8,
}

#[allow(dead_code)]
fn default_sort() -> u8 {
  5
}

impl Item {
  /// Convenience: set `sort_priority` in a builder-ish way.
  pub fn with_sort(mut self, p: u8) -> Self {
    self.sort_priority = p;
    self
  }

  /// Markdown documentation for this item, rendering the knowledge-base
  /// entry on demand. The single accessor callers should use --
  /// `documentation_md` alone misses every built-in.
  pub fn documentation(&self) -> Option<String> {
    match (&self.documentation_md, self.kb_entry) {
      (Some(md), _) => Some(md.clone()),
      (None, Some(e)) => Some(dsl_knowledge::render_markdown(e)),
      (None, None) => None,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
pub enum ItemKind {
  #[default]
  Keyword,
  Type,
  Function,
  Table,
  View,
  Column,
  Schema,
  Variable,
  Parameter,
}
