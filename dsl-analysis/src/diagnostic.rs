//! Diagnostic shape returned by every rule.

use serde::Serialize;
use text_size::TextRange;

/// Build a [`TextRange`] from two byte-offset `usize` values without
/// the `(_ as u32).into()` boilerplate. Used by every rule when
/// converting `start..end` byte spans into the LSP-facing range.
#[inline]
pub fn range_at(start: usize, end: usize) -> TextRange {
  TextRange::new((start as u32).into(), (end as u32).into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
  Error,
  Warning,
  Info,
  Hint,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
  pub code: &'static str,
  pub severity: Severity,
  pub message: String,
  pub range: TextRange,
}
