//! sql323: `SELECT ... FROM DUAL` -- Oracle's dummy single-row
//! table. PG doesn't have DUAL; `SELECT 1;` works without FROM.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql323"
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
    let Some(at) = upper.find(" FROM DUAL") else { return };
    // Word boundary on DUAL.
    let after = at + " FROM DUAL".len();
    if after < upper.len() {
      let next = upper.as_bytes()[after] as char;
      if next.is_ascii_alphanumeric() || next == '_' { return }
    }
    let abs_s = start + at + " FROM ".len();
    let abs_e = abs_s + "DUAL".len();
    out.push(Diagnostic {
      code: "sql323",
      severity: Severity::Error,
      message: "`FROM DUAL` is Oracle syntax -- PG allows `SELECT <expr>;` without FROM".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
