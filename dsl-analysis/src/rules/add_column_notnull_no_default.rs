//! sql248: `ALTER TABLE t ADD COLUMN c <type> NOT NULL` (no
//! DEFAULT). On PG<11 PG rewrites the whole table to fill the new
//! column -- AccessExclusiveLock for the duration, which is risky
//! on big tables. On PG11+ a constant DEFAULT avoids the rewrite,
//! but NOT NULL alone with no default still fails if any row
//! already exists. Hint: add a DEFAULT or split into two steps
//! (ADD nullable -> backfill -> ALTER SET NOT NULL).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql248"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("ALTER TABLE") {
      return;
    }
    if !upper.contains("ADD COLUMN") {
      return;
    }
    if !upper.contains("NOT NULL") {
      return;
    }
    if upper.contains("DEFAULT") {
      return;
    }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql248",
      severity: Severity::Warning,
      message: "ALTER TABLE ADD COLUMN ... NOT NULL with no DEFAULT -- fails on non-empty tables; add a DEFAULT or split into ADD nullable + backfill + SET NOT NULL".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
