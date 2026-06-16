//! sql595: `REPLACE INTO t ...` -- MySQL's REPLACE statement (a DELETE of any
//! conflicting row followed by an INSERT). PostgreSQL has no REPLACE; use
//! `INSERT ... ON CONFLICT (<cols>) DO UPDATE SET ...` for a true upsert, or an
//! explicit DELETE + INSERT if you really want the delete-then-insert
//! semantics (which also fire ON DELETE triggers / cascades).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql595"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let u = upper.trim_start();
    let lead = upper.len() - u.len();
    // Leading REPLACE keyword (statement form), then INTO -- not the
    // `replace(str, from, to)` function.
    if !(u.starts_with("REPLACE") && u.as_bytes().get(7).is_none_or(|&b| !is_word(b as char))) {
      return;
    }
    let rest = u[7..].trim_start();
    if !rest.to_ascii_uppercase().starts_with("INTO") {
      return;
    }
    out.push(Diagnostic {
      code: "sql595",
      severity: Severity::Error,
      message: "`REPLACE INTO` is MySQL syntax -- PostgreSQL uses `INSERT ... ON CONFLICT (cols) DO UPDATE SET ...`".into(),
      range: crate::range_at(start + lead, start + lead + 7),
    });
  }
}
