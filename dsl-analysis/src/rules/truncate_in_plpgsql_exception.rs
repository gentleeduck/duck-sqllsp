//! sql339: `TRUNCATE` inside a PL/pgSQL function body that also has an
//! `EXCEPTION` block. PL/pgSQL EXCEPTION wraps the body in a subxact;
//! TRUNCATE acquires an ACCESS EXCLUSIVE lock that doesn't roll back
//! cleanly inside subxacts and can leave the catalog in a state where
//! the row visibility is wrong for the rest of the transaction. Hint.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql339"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Require a function/procedure body marker, EXCEPTION, and TRUNCATE.
    let in_fn = upper.contains("CREATE OR REPLACE FUNCTION")
      || upper.contains("CREATE FUNCTION")
      || upper.contains("CREATE OR REPLACE PROCEDURE")
      || upper.contains("CREATE PROCEDURE")
      || upper.contains("DO $$");
    if !in_fn {
      return;
    }
    if !upper.contains("EXCEPTION") {
      return;
    }
    let Some(at) = upper.find("TRUNCATE") else { return };
    let abs_s = start + at;
    let abs_e = abs_s + "TRUNCATE".len();
    out.push(Diagnostic {
      code: "sql339",
      severity: Severity::Warning,
      message: "TRUNCATE inside a PL/pgSQL body with EXCEPTION risks visibility issues -- the ACCESS EXCLUSIVE lock + subxact rollback don't compose cleanly".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
