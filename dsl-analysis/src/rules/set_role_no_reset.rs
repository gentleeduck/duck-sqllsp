//! sql135: `SET ROLE x` inside a transaction without a matching
//! `RESET ROLE` -- the elevated role leaks past the COMMIT into the
//! pooled connection's lifetime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql135"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let end = start + body.len();
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("SET ROLE ") && trimmed != "SET ROLE" {
      return;
    }
    // Already SET ROLE NONE / RESET? Skip.
    if trimmed.contains("NONE") {
      return;
    }
    // Look forward for RESET ROLE or SET ROLE NONE within the same
    // BEGIN..COMMIT window. If we find COMMIT/ROLLBACK first, the
    // role wasn't reset.
    let after_upper = source[end..].to_ascii_uppercase();
    let reset_at = after_upper.find("RESET ROLE").or_else(|| after_upper.find("SET ROLE NONE"));
    let end_tx_at = after_upper.find("COMMIT").or_else(|| after_upper.find("ROLLBACK"));
    let needs_warn = match (reset_at, end_tx_at) {
      (Some(_r), None) => false,
      (Some(r), Some(c)) => r > c,
      (None, _) => true,
    };
    if !needs_warn {
      return;
    }
    let leading = upper.len() - trimmed.len();
    let abs_start = start + leading;
    let abs_end = abs_start + 8;
    out.push(Diagnostic {
      code: "sql135",
      severity: Severity::Hint,
      message:
        "SET ROLE without matching RESET ROLE -- the elevated role outlives the transaction on pooled connections"
          .into(),
      range: crate::range_at(abs_start, abs_end),
    });
  }
}
