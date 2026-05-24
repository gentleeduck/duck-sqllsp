//! Diagnostic shape returned by every rule.

use serde::Serialize;
use text_size::TextRange;

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
