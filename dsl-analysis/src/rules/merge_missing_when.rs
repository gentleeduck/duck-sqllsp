//! sql261: `MERGE INTO t USING src ON ... ;` -- needs at least one
//! WHEN MATCHED / WHEN NOT MATCHED clause; PG raises 42601 at parse.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql261"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("MERGE") { return }
    if upper.contains("WHEN MATCHED") || upper.contains("WHEN NOT MATCHED") { return }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql261",
      severity: Severity::Error,
      message: "MERGE without any WHEN MATCHED / WHEN NOT MATCHED -- PG raises 42601; add at least one action".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
