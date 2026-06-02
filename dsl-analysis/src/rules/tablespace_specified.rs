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
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(at) = upper.find("TABLESPACE ") else { return };
    // Skip the SET default_tablespace = '...' form (no privilege issue).
    let prev_upper = &upper[..at];
    if prev_upper.ends_with("DEFAULT_") {
      return;
    }
    let abs_s = start + at;
    let abs_e = abs_s + "TABLESPACE".len();
    out.push(Diagnostic {
      code: "sql335",
      severity: Severity::Hint,
      message: "explicit TABLESPACE clauses require CREATE-in-tablespace privilege and are rejected by most managed PG offerings".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
