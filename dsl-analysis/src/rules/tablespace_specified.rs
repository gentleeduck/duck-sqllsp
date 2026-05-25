//! sql335: explicit `TABLESPACE <name>` clause in a buffer that
//! likely runs as a non-superuser migration. PG only allows
//! TABLESPACE on objects the caller owns + can create-in-tblspc;
//! cloud-hosted PG usually rejects non-default tablespaces outright.
//! Hint that this will break in many deployment targets.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql335"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(at) = upper.find("TABLESPACE ") else { return };
    // Skip the SET default_tablespace = '...' form (no privilege issue).
    let prev_upper = &upper[..at];
    if prev_upper.ends_with("DEFAULT_") { return }
    let abs_s = start + at;
    let abs_e = abs_s + "TABLESPACE".len();
    out.push(Diagnostic {
      code: "sql335",
      severity: Severity::Hint,
      message: "explicit TABLESPACE clauses require CREATE-in-tablespace privilege and are rejected by most managed PG offerings".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
