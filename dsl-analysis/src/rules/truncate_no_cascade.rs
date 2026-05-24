//! sql105: `TRUNCATE t` without `CASCADE` -- if any FK points at `t`,
//! the command fails at runtime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql105"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Only inspect TRUNCATE statements.
    let trimmed = upper.trim_start();
    let leading_ws = upper.len() - trimmed.len();
    if !trimmed.starts_with("TRUNCATE") {
      return;
    }
    // Already CASCADE or RESTRICT? Skip.
    if upper.contains(" CASCADE") || upper.contains(" RESTRICT") {
      return;
    }
    let abs_start = start + leading_ws;
    let abs_end = start + leading_ws + 8;
    out.push(Diagnostic {
      code: "sql105",
      severity: Severity::Hint,
      message: "TRUNCATE without CASCADE -- will fail at runtime if any FK references this table".into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}
