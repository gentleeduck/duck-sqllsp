//! sql295: `COPY ... WITH (HEADER, FORMAT TEXT)` -- the HEADER
//! option is only valid for CSV format. PG raises 42601 at parse.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql295"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("COPY") { return }
    if !upper.contains("HEADER") { return }
    let csv = upper.contains("CSV") || upper.contains("FORMAT CSV") || upper.contains("FORMAT 'CSV'");
    if csv { return }
    let Some(at) = upper.find("HEADER") else { return };
    let abs_s = start + at;
    let abs_e = abs_s + "HEADER".len();
    out.push(Diagnostic {
      code: "sql295",
      severity: Severity::Error,
      message: "COPY HEADER is only valid for CSV format -- add `WITH (FORMAT CSV, HEADER true)` or drop HEADER".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
