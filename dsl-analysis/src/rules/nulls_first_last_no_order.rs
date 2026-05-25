//! sql231: `NULLS FIRST` / `NULLS LAST` outside an ORDER BY clause.
//! PG raises 42601 at parse time. Catches the pattern where the
//! author wrote a DISTINCT or SELECT clause and bolted NULLS FIRST
//! on by mistake.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql231"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    for needle in ["NULLS FIRST", "NULLS LAST"] {
      let Some(at) = upper.find(needle) else { continue };
      // Must be preceded by ORDER BY somewhere earlier.
      let before = &upper[..at];
      if before.contains("ORDER BY") { continue }
      // Inside a window function frame? Skip if WINDOW preceded.
      if before.contains("WINDOW ") { continue }
      // CREATE INDEX accepts NULLS FIRST/LAST as a column modifier.
      if before.contains("CREATE INDEX") || before.contains("CREATE UNIQUE INDEX") { continue }
      let abs_s = start + at;
      let abs_e = abs_s + needle.len();
      out.push(Diagnostic {
        code: "sql231",
        severity: Severity::Error,
        message: format!("`{needle}` outside ORDER BY -- valid only as an ORDER BY modifier (PG 42601)"),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
