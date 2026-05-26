//! sql300: `SELECT a, b, FROM t` -- trailing comma in projection
//! list. PG raises 42601 at parse. Catches the very common typo
//! from copy-pasting projection items.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql300"
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
    let Some(sel_at) = upper.find("SELECT ") else { return };
    let after_sel = sel_at + "SELECT ".len();
    let Some(from_rel) = upper[after_sel..].find(" FROM ") else { return };
    let proj_end = after_sel + from_rel;
    let proj = body[after_sel..proj_end].trim_end();
    if !proj.ends_with(',') { return }
    let abs_s = start + after_sel + (body[after_sel..proj_end].trim_end_matches(',').len());
    let abs_e = start + proj_end;
    out.push(Diagnostic {
      code: "sql300",
      severity: Severity::Error,
      message: "Trailing comma in SELECT projection -- PG raises 42601 (`syntax error at or near \"FROM\"`)".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
