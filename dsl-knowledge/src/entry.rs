//! Shape of a single knowledge-base entry.

use serde::Serialize;

/// One entry in the knowledge base.
///
/// `signature` is set only for functions; for keywords and types it stays
/// `None` because they don't have a meaningful arity.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
  /// Display label in canonical case (`"SELECT"`, `"now"`, `"UUID"`).
  pub label: &'static str,
  /// What this entry represents. Drives the LSP completion item kind.
  pub kind: Kind,
  /// One-line semantic description.
  pub doc: &'static str,
  /// Optional formal signature, only for functions.
  pub signature: Option<&'static str>,
  /// Short SQL snippet showing canonical usage.
  pub example: &'static str,
  /// Link to the canonical Postgres documentation page.
  pub url: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Kind {
  Keyword,
  Type,
  Function,
}

/// Canonical Postgres docs base URL. Every `Entry::url` resolves against
/// this string so URLs stay internally consistent.
pub const PG_DOCS_BASE: &str = "https://www.postgresql.org/docs/current/";

/// Helper used by the per-table seed functions to build a Postgres docs URL
/// from a relative path fragment.
pub(crate) fn pg(path: &'static str) -> &'static str {
  // Allocate once at first call and leak. The table is built once and
  // never freed, so leaking is exactly the right ownership shape.
  Box::leak(format!("{PG_DOCS_BASE}{path}").into_boxed_str())
}
