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

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else { return };
    if ct.table.schema.is_some() { return }
    // Only flag when the buffer ALREADY uses schema qualifiers on at
    // least one other CREATE TABLE -- mixing the two forms is the
    // real smell. Flat-schema files (no schema prefix anywhere) get
    // a pass; flagging every CREATE TABLE in that style is pure noise.
    if !buffer_has_qualified_create(source) { return }
    out.push(Diagnostic {
      code: "sql327",
      severity: Severity::Hint,
      message: format!("CREATE TABLE `{}` has no schema qualifier -- other tables in this buffer ARE qualified; pick one style", ct.table.name),
      range: stmt.range,
    });
  }
}

/// Walk the buffer for `CREATE TABLE <schema>.<name>` -- evidence that
/// the author uses schema qualifiers and just forgot on this one.
fn buffer_has_qualified_create(source: &str) -> bool {
  let upper = source.to_ascii_uppercase();
  let bytes = source.as_bytes();
  for needle in ["CREATE TABLE IF NOT EXISTS ", "CREATE TEMP TABLE ", "CREATE TEMPORARY TABLE ", "CREATE UNLOGGED TABLE ", "CREATE TABLE "] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(needle) {
      let at = from + rel + needle.len();
      let mut k = at;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1 }
      let id_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') {
        k += 1;
      }
      let id = &source[id_start..k];
      if id.contains('.') { return true }
      from = k;
    }
  }
  false
}
