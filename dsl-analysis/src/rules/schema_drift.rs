//! sql173: workspace CREATE TABLE diverges from the live catalog.
//!
//! When the user has a live connection AND a CREATE TABLE for the
//! same name in the buffer, compare column sets. Columns in the
//! buffer but missing from live -> drift error on the missing-side.
//! Columns in live but missing from buffer -> hint that the table
//! has extra columns the DDL doesn't declare.
//!
//! Skips when no live catalog (live tables empty) OR no buffer
//! CreateTable matches.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql173"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, _source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else { return };
    // PARTITION OF / LIKE-only / inheritance forms have no explicit
    // column list -- comparing zero buffer cols against the inherited
    // live cols would always report drift. Skip.
    if ct.columns.is_empty() { return }
    // Heuristic for "live catalog present": at least one table has a
    // non-empty constraint or index list. The source-derived offline
    // catalog has none of those.
    let live_present = catalog
      .tables()
      .any(|t| !t.constraints.is_empty() || !t.indexes.is_empty() || !t.triggers.is_empty());
    if !live_present {
      return;
    }
    let Some(live) = catalog.find_table(ct.table.schema.as_deref(), &ct.table.name) else { return };
    let buffer_cols: std::collections::HashSet<String> =
      ct.columns.iter().map(|c| c.name.to_ascii_lowercase()).collect();
    let live_cols: std::collections::HashSet<String> =
      live.columns.iter().map(|c| c.name.to_ascii_lowercase()).collect();
    let missing_in_live: Vec<&String> = buffer_cols.difference(&live_cols).collect();
    let extra_in_live: Vec<&String> = live_cols.difference(&buffer_cols).collect();
    if missing_in_live.is_empty() && extra_in_live.is_empty() {
      return;
    }
    let mut bits = Vec::new();
    if !missing_in_live.is_empty() {
      bits.push(format!("missing in live: {}", joined_sorted(&missing_in_live)));
    }
    if !extra_in_live.is_empty() {
      bits.push(format!("extra in live: {}", joined_sorted(&extra_in_live)));
    }
    out.push(Diagnostic {
      code: "sql173",
      severity: Severity::Hint,
      message: format!("schema drift on `{}`: {}", ct.table.name, bits.join("; ")),
      range: stmt.range,
    });
  }
}

fn joined_sorted(items: &[&String]) -> String {
  let mut v: Vec<&String> = items.iter().copied().collect();
  v.sort();
  v.into_iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
}
