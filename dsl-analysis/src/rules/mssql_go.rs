//! sql321: standalone `GO` -- MSSQL batch separator. PG raises
//! 42601 (`syntax error at or near "GO"`).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql321"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim();
    if trim != "GO" && !trim.starts_with("GO\n") && !trim.starts_with("GO ") { return }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = abs_s + 2;
    out.push(Diagnostic {
      code: "sql321",
      severity: Severity::Error,
      message: "`GO` is the MSSQL batch separator -- PG raises 42601; remove it or split files".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
