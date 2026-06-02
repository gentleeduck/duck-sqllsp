//! sql328: REVOKE in a buffer that has no matching GRANT.
//!
//! Style/safety: a stand-alone REVOKE migration depends on whoever
//! ran the original GRANT. When the buffer contains the GRANT/REVOKE
//! pair the intent is obvious; a lone REVOKE usually means the
//! migration author has assumed a prior state that may not hold.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql328"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, end) = crate::stmt_bounds(stmt, source);
    let body = source[start..end].trim_start();
    let upper = body.to_ascii_uppercase();
    if !upper.starts_with("REVOKE") {
      return;
    }
    // Bail if the full buffer also has a GRANT.
    if source.to_ascii_uppercase().contains("GRANT ") {
      return;
    }
    let abs_s = start + (body.len() - body.trim_start().len());
    let abs_e = abs_s + 6;
    out.push(Diagnostic {
      code: "sql328",
      severity: Severity::Hint,
      message: "REVOKE with no matching GRANT in this buffer -- the prior state may not hold".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
