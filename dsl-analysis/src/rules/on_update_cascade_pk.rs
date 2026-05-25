//! sql333: `ON UPDATE CASCADE` on a column referenced as a primary key.
//!
//! ON UPDATE CASCADE is rarely the right choice on a PK column --
//! PK values are supposed to be immutable. Almost always means the
//! author confused ON UPDATE with ON DELETE intent. Warn.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql333"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("PRIMARY KEY") { return }
    let Some(on_at) = upper.find("ON UPDATE CASCADE") else { return };
    let abs_s = start + on_at;
    let abs_e = abs_s + "ON UPDATE CASCADE".len();
    out.push(Diagnostic {
      code: "sql333",
      severity: Severity::Warning,
      message: "ON UPDATE CASCADE on a PRIMARY KEY column is rarely intended -- PK values should be immutable".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
