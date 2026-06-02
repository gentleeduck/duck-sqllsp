//! sql318: `SELECT TOP 10 ...` -- MSSQL/Sybase syntax. PG uses
//! `SELECT ... LIMIT 10`. Catches a common port mistake.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql318"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let trim = upper.trim_start();
    if !trim.starts_with("SELECT TOP ") && !trim.starts_with("SELECT TOP(") {
      return;
    }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead + "SELECT ".len();
    let abs_e = abs_s + "TOP".len();
    out.push(Diagnostic {
      code: "sql318",
      severity: Severity::Error,
      message: "`SELECT TOP N` is MSSQL/Sybase syntax -- PG uses `SELECT ... LIMIT N`".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
