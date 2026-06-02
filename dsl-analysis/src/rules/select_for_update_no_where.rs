//! sql072: `SELECT ... FOR UPDATE` without a WHERE clause locks every
//! row of the target table -- almost always a footgun.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql072"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Select(_)) {
      return;
    }
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let (clause, idx) = if let Some(p) = upper.find("FOR UPDATE") {
      ("FOR UPDATE", p)
    } else if let Some(p) = upper.find("FOR SHARE") {
      ("FOR SHARE", p)
    } else {
      return;
    };
    if crate::textutil::contains_word(&upper, "WHERE") {
      return;
    }
    let abs_start = start + idx;
    let abs_end = abs_start + clause.len();
    out.push(Diagnostic {
      code: "sql072",
      severity: Severity::Warning,
      message: "SELECT FOR UPDATE / FOR SHARE without WHERE -- locks every row in the table".into(),
      range: crate::range_at(abs_start, abs_end),
    });
  }
}
