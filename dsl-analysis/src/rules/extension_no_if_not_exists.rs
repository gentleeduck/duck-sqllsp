//! sql262: `CREATE EXTENSION pg_stat_statements` (without IF NOT
//! EXISTS). Migration scripts almost always want the idempotent
//! form. Hint: add IF NOT EXISTS.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql262"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim_start();
    if !trim.starts_with("CREATE EXTENSION") { return }
    if upper.contains("IF NOT EXISTS") { return }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql262",
      severity: Severity::Hint,
      message: "CREATE EXTENSION without IF NOT EXISTS -- migration breaks on rerun; prefer the idempotent form".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
