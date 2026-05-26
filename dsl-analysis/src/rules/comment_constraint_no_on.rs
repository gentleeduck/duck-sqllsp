//! sql279: `COMMENT ON CONSTRAINT pk_users IS '...'` -- needs the
//! `ON <table>` qualifier (e.g. `ON users`). PG raises 42601 at
//! parse time without it.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql279"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(at) = upper.find("COMMENT ON CONSTRAINT ") else { return };
    let after = at + "COMMENT ON CONSTRAINT ".len();
    let rest = &body[after..];
    let rest_upper = &upper[after..];
    let Some(is_at) = rest_upper.find(" IS ") else { return };
    let between = &rest_upper[..is_at];
    if between.contains(" ON ") { return }
    let abs_s = start + at;
    let abs_e = abs_s + ("COMMENT ON CONSTRAINT ".len() + is_at);
    out.push(Diagnostic {
      code: "sql279",
      severity: Severity::Error,
      message: "COMMENT ON CONSTRAINT requires `ON <table>` qualifier -- PG raises 42601 without it".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
    let _ = rest;
  }
}
