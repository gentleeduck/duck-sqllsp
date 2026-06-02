//! sql322: `BEGIN TRAN` -- MSSQL shorthand for BEGIN TRANSACTION.
//! PG only accepts `BEGIN`, `BEGIN TRANSACTION`, or `BEGIN WORK`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql322"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let trim = upper.trim_start();
    // Match "BEGIN TRAN" but not "BEGIN TRANSACTION".
    if !trim.starts_with("BEGIN TRAN") {
      return;
    }
    if trim.starts_with("BEGIN TRANSACTION") {
      return;
    }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = abs_s + "BEGIN TRAN".len();
    out.push(Diagnostic {
      code: "sql322",
      severity: Severity::Error,
      message: "`BEGIN TRAN` is MSSQL shorthand -- PG needs `BEGIN`, `BEGIN TRANSACTION`, or `BEGIN WORK`".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
