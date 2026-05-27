//! sql402: duplicate FROM/JOIN alias in a single SELECT.
//!
//! Example: `SELECT * FROM users a, orders a` -- PG rejects this with
//! "table name 'a' specified more than once". We catch it earlier so
//! the user fixes the alias before running.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use std::collections::HashSet;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql402"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, _source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
    // Walk FROM + JOIN bindings and collect their effective alias
    // names. A binding without an explicit alias falls back to the bare
    // table name (PG follows the same convention).
    let mut seen: HashSet<String> = HashSet::new();
    let mut emitted: HashSet<String> = HashSet::new();
    let iter = s.from.iter().chain(s.joins.iter().map(|j| &j.table));
    for t in iter {
      let alias = t.alias.clone().unwrap_or_else(|| t.name.clone());
      let key = alias.to_ascii_lowercase();
      if key.is_empty() {
        continue;
      }
      if !seen.insert(key.clone()) && emitted.insert(key) {
        out.push(Diagnostic {
          code: "sql402",
          severity: Severity::Error,
          message: format!("duplicate alias `{alias}` in FROM/JOIN -- table names must be unique within a single SELECT"),
          range: stmt.range,
        });
      }
    }
  }
}
