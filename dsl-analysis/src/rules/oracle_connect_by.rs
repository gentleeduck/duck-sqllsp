//! sql325: `CONNECT BY PRIOR ...` -- Oracle hierarchical query.
//! PG has no CONNECT BY; use `WITH RECURSIVE` instead.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql325"
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
    let Some(at) = upper.find("CONNECT BY") else { return };
    let abs_s = start + at;
    let abs_e = abs_s + "CONNECT BY".len();
    out.push(Diagnostic {
      code: "sql325",
      severity: Severity::Error,
      message: "`CONNECT BY` is Oracle hierarchical query syntax -- PG uses `WITH RECURSIVE` for tree/graph traversal"
        .into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
