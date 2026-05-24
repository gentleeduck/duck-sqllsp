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
    pub documentation_md: Option<String>,
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

fn default_sort() -> u8 { 5 }

impl Item {
    /// Convenience: set `sort_priority` in a builder-ish way.
    pub fn with_sort(mut self, p: u8) -> Self { self.sort_priority = p; self }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
pub enum ItemKind {
    #[default]
    Keyword,
    Type, Function, Table, View, Column, Schema, Variable, Parameter,
}
