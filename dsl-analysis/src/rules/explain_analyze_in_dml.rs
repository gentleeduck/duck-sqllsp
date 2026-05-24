//! sql125: `EXPLAIN ANALYZE INSERT/UPDATE/DELETE` -- ANALYZE actually
//! runs the query so the DML mutates the table. Often surprises people
//! debugging in prod. Suggest wrapping in BEGIN ... ROLLBACK.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql125"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("EXPLAIN") {
      return;
    }
    // Match `EXPLAIN ANALYZE` or `EXPLAIN (ANALYZE, ...)`.
    let has_analyze = trimmed.contains("ANALYZE");
    if !has_analyze {
      return;
    }
    // Look for DML inside the explained body.
    let is_dml = upper.contains("INSERT INTO")
      || upper.contains("UPDATE ")
      || upper.contains("DELETE FROM")
      || upper.contains("MERGE INTO");
    if !is_dml {
      return;
    }
    let leading_ws = upper.len() - trimmed.len();
    let abs_start = start + leading_ws;
    let abs_end = start + leading_ws + 7;
    out.push(Diagnostic {
      code: "sql125",
      severity: Severity::Hint,
      message: "EXPLAIN ANALYZE on DML executes the statement -- wrap in BEGIN ... ROLLBACK if you only want the plan"
        .into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}
