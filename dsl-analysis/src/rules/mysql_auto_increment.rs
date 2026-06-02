//! sql314: `AUTO_INCREMENT` -- MySQL column attribute. PG has no
//! AUTO_INCREMENT; use `SERIAL` / `BIGSERIAL` (legacy) or
//! `GENERATED ALWAYS AS IDENTITY` (preferred, PG10+).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql314"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(at) = upper.find("AUTO_INCREMENT") else { return };
    if at > 0 {
      let prev = body.as_bytes()[at - 1] as char;
      if prev.is_ascii_alphanumeric() || prev == '_' {
        return;
      }
    }
    let abs_s = start + at;
    let abs_e = abs_s + "AUTO_INCREMENT".len();
    out.push(Diagnostic {
      code: "sql314",
      severity: Severity::Error,
      message: "AUTO_INCREMENT is MySQL syntax -- PG uses `SERIAL` / `BIGSERIAL` (legacy) or `GENERATED ALWAYS AS IDENTITY` (PG10+, recommended)".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
