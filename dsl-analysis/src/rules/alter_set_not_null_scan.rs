//! sql281: `ALTER TABLE t ALTER COLUMN c SET NOT NULL` -- PG scans
//! every row to verify nullability + holds AccessExclusiveLock. On
//! big tables: outage. Recommended pattern: add CHECK (c IS NOT
//! NULL) NOT VALID, validate it in the background, then SET NOT NULL
//! (which on PG12+ short-circuits when an equivalent CHECK is
//! already VALID).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql281"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("ALTER TABLE") { return }
    if !upper.contains("SET NOT NULL") { return }
    let Some(at) = upper.find("SET NOT NULL") else { return };
    let abs_s = start + at;
    let abs_e = abs_s + "SET NOT NULL".len();
    out.push(Diagnostic {
      code: "sql281",
      severity: Severity::Hint,
      message: "ALTER COLUMN SET NOT NULL scans table under AccessExclusiveLock -- on big tables: ADD CHECK (col IS NOT NULL) NOT VALID, VALIDATE CONSTRAINT, then SET NOT NULL (PG12+ avoids the second scan)".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
