//! sql302: `DROP TABLE foo` (or DROP INDEX/VIEW/TRIGGER/etc)
//! without `IF EXISTS`. Migrations and rollback scripts almost
//! always want the idempotent form; otherwise rerun raises 42P01.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql302"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim_start();
    let droppable = [
      "DROP TABLE", "DROP INDEX", "DROP VIEW", "DROP MATERIALIZED VIEW",
      "DROP TRIGGER", "DROP TYPE", "DROP DOMAIN", "DROP SEQUENCE",
      "DROP FUNCTION", "DROP PROCEDURE", "DROP SCHEMA",
    ];
    if !droppable.iter().any(|d| trim.starts_with(d)) { return }
    if upper.contains("IF EXISTS") { return }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql302",
      severity: Severity::Hint,
      message: "DROP without IF EXISTS -- migration / rollback fails on rerun; prefer the idempotent form".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
