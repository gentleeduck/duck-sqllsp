//! sql242: `DROP SCHEMA foo` (no CASCADE / RESTRICT) -- PG defaults
//! to RESTRICT and fails with 2BP01 "schema X is not empty" when
//! any object lives inside. Make it explicit so the author confirms
//! their intent.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql242"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let trim = upper.trim_start();
    if !(trim.starts_with("DROP SCHEMA") || trim.starts_with("DROP DATABASE")) {
      return;
    }
    if upper.contains("CASCADE") || upper.contains("RESTRICT") {
      return;
    }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql242",
      severity: Severity::Hint,
      message: "DROP SCHEMA/DATABASE without CASCADE or RESTRICT -- default RESTRICT; if intent is to drop contents too, add CASCADE explicitly".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
