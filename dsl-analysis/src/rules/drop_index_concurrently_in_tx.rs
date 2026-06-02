//! sql331: `DROP INDEX CONCURRENTLY` inside an explicit transaction.
//!
//! Like `CREATE INDEX CONCURRENTLY`, the CONCURRENTLY drop variant
//! cannot run inside a BEGIN/COMMIT block. PG raises 25001 at runtime.
//! Flag when the same buffer mixes a CONCURRENTLY drop with a BEGIN.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql331"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, end) = crate::stmt_bounds(stmt, source);
    let body = &source[start..end].trim_start();
    let upper = body.to_ascii_uppercase();
    if !(upper.starts_with("DROP INDEX CONCURRENTLY") || upper.starts_with("DROP INDEX IF EXISTS CONCURRENTLY")) {
      return;
    }
    // Buffer has an explicit BEGIN before this statement?
    let prefix_upper = source[..start].to_ascii_uppercase();
    if !prefix_upper.contains("BEGIN") && !prefix_upper.contains("START TRANSACTION") {
      return;
    }
    let abs_s = start + (source[start..].len() - source[start..].trim_start().len());
    let abs_e = (abs_s + "DROP INDEX CONCURRENTLY".len()).min(end);
    out.push(Diagnostic {
      code: "sql331",
      severity: Severity::Error,
      message: "DROP INDEX CONCURRENTLY cannot run inside a transaction (25001) -- move it out of BEGIN/COMMIT".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
