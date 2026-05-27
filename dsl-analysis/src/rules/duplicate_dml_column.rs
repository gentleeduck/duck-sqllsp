//! sql406: duplicate column in an INSERT column list or UPDATE SET
//! assignment list.
//!
//! - `INSERT INTO t (a, b, a) VALUES (...)` -- PG: column "a" specified more than once
//! - `UPDATE t SET a = 1, a = 2` -- PG: multiple assignments to same column "a"
//!
//! Both forms are pure typos and the user almost certainly meant to
//! reference two different columns, so we surface them as errors.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use std::collections::HashSet;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql406"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, _source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    match &stmt.kind {
      StatementKind::Insert(i) => {
        emit_duplicates(&i.columns, "INSERT column list", stmt, out);
      },
      StatementKind::Update(u) => {
        let cols: Vec<String> = u.assignments.iter().map(|(c, _)| c.clone()).collect();
        emit_duplicates(&cols, "UPDATE SET", stmt, out);
      },
      _ => {},
    }
  }
}

fn emit_duplicates(cols: &[String], where_: &str, stmt: &Statement, out: &mut Vec<Diagnostic>) {
  let mut seen: HashSet<String> = HashSet::new();
  let mut emitted: HashSet<String> = HashSet::new();
  for c in cols {
    if c.is_empty() {
      continue;
    }
    let key = c.to_ascii_lowercase();
    if !seen.insert(key.clone()) && emitted.insert(key) {
      out.push(Diagnostic {
        code: "sql406",
        severity: Severity::Error,
        message: format!("duplicate column `{c}` in {where_} -- PG rejects this with `column specified more than once`"),
        range: stmt.range,
      });
    }
  }
}
