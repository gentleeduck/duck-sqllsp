//! sql327: `CREATE TABLE foo (...)` without an explicit schema qualifier.
//!
//! Style hint: every CREATE TABLE in a multi-schema project should
//! spell out which schema the table belongs to. Otherwise the table
//! lands in whatever `search_path` happens to be first -- usually
//! `public`, but breaks if a migration runs with a different default.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql327"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, _source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else { return };
    if ct.table.schema.is_some() { return }
    out.push(Diagnostic {
      code: "sql327",
      severity: Severity::Hint,
      message: format!("CREATE TABLE `{}` has no schema qualifier -- consider `schema.{}`", ct.table.name, ct.table.name),
      range: stmt.range,
    });
  }
}
