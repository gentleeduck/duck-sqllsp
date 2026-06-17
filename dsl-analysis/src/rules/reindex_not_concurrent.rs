//! sql590: a `REINDEX` without `CONCURRENTLY`. Plain REINDEX takes an ACCESS
//! EXCLUSIVE lock on the table (or index's table), blocking all reads and
//! writes until the rebuild finishes. Since PG12, `REINDEX INDEX CONCURRENTLY`
//! / `REINDEX TABLE CONCURRENTLY` rebuild online with only a SHARE UPDATE
//! EXCLUSIVE lock.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql590"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let u = upper.trim_start();
    let lead = upper.len() - u.len();
    if !u.starts_with("REINDEX") || u.as_bytes().get(7).is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_') {
      return;
    }
    if upper.contains("CONCURRENTLY") {
      return;
    }
    out.push(Diagnostic {
      code: "sql590",
      severity: Severity::Warning,
      message: "REINDEX without CONCURRENTLY locks the table against all reads and writes -- use REINDEX ... CONCURRENTLY (PG12+) to rebuild online".into(),
      range: crate::range_at(start + lead, start + lead + 7),
    });
  }
}
