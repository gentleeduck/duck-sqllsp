//! sql212: top-level `SELECT * INTO foo FROM bar` -- DDL form creates
//! a NEW table `foo`. If `foo` already exists in the catalog, PG
//! raises 42P07 at runtime. Skip when inside a $$...$$ body (SELECT
//! INTO inside PL/pgSQL is an assignment, handled by sql118).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql212"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Skip plpgsql bodies entirely.
    if inside_dollar_block(source, stmt) { return }
    if !matches!(&stmt.kind, StatementKind::Select(_) | StatementKind::Unknown { .. }) { return }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(into_at) = upper.find(" INTO ") else { return };
    // Only fire when the INTO is owned by a SELECT, not an INSERT.
    // For an Unknown stmt that wraps a CREATE FUNCTION body, we may
    // see `INSERT INTO ...` inside the body -- those are normal
    // INSERTs, not the DDL-form `SELECT * INTO foo`.
    let prefix_upper = &upper[..into_at];
    let last_select = prefix_upper.rfind("SELECT");
    let last_insert = prefix_upper.rfind("INSERT");
    let select_owns = match (last_select, last_insert) {
      (Some(s), Some(i)) => s > i,
      (Some(_), None) => true,
      _ => false,
    };
    if !select_owns { return }
    let after = into_at + " INTO ".len();
    let rest = body[after..].trim_start();
    let upper_rest = rest.to_ascii_uppercase();
    if upper_rest.starts_with("STRICT") { return }
    let mut effective_rest = rest;
    if upper_rest.starts_with("TEMP ") || upper_rest.starts_with("TEMPORARY ") || upper_rest.starts_with("UNLOGGED ") {
      // Temp tables are session-scoped; never clash with catalog -> skip.
      return
    }
    let id_end = effective_rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(effective_rest.len());
    let name = effective_rest[..id_end].trim_matches('"');
    if name.is_empty() { return }
    let bare = name.rsplit('.').next().unwrap_or(name);
    if catalog.find_table(None, bare).is_none() { return }
    effective_rest = &effective_rest[..id_end];
    let _ = effective_rest;
    let abs_s = start + after + (rest.len() - rest.trim_start().len());
    let abs_e = abs_s + id_end;
    out.push(Diagnostic {
      code: "sql212",
      severity: Severity::Error,
      message: format!(
        "SELECT INTO `{bare}` -- DDL form creates a NEW table; `{bare}` already exists in catalog, PG raises 42P07"
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn inside_dollar_block(source: &str, stmt: &Statement) -> bool {
  let start: usize = u32::from(stmt.range.start()) as usize;
  let prelude = &source[..start];
  let cnt = prelude.matches("$$").count();
  cnt % 2 == 1
}
