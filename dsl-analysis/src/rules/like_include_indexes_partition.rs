//! sql338: `CREATE TABLE x PARTITION OF parent (LIKE base INCLUDING INDEXES ...)`
//!
//! INCLUDING INDEXES inside a PARTITION OF body is silently ignored by
//! PG: partition tables can't declare independent indexes that way --
//! the parent's index template attaches them. Flag so the author knows
//! the clause is a no-op.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql338"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("PARTITION OF") {
      return;
    }
    let Some(at) = upper.find("INCLUDING INDEXES") else { return };
    let abs_s = start + at;
    let abs_e = abs_s + "INCLUDING INDEXES".len();
    out.push(Diagnostic {
      code: "sql338",
      severity: Severity::Hint,
      message: "INCLUDING INDEXES is ignored inside PARTITION OF -- the parent table's index template controls partition indexes".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
