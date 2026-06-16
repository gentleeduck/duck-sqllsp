//! sql636: the SQLite `AUTOINCREMENT` keyword (one word, e.g.
//! `id INTEGER PRIMARY KEY AUTOINCREMENT`). PostgreSQL doesn't accept it; use
//! `GENERATED ALWAYS AS IDENTITY` (preferred) or `serial`/`bigserial`. Note a
//! plain `bigint PRIMARY KEY GENERATED ... AS IDENTITY` already auto-assigns
//! without SQLite's rowid-reuse semantics. Sibling of the MySQL AUTO_INCREMENT
//! lint (sql314).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql636"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let needle = b"AUTOINCREMENT";
    let len = needle.len();
    let mut i = 0usize;
    while i + len <= n {
      if &ub[i..i + len] == needle
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + len == n || !is_word(ub[i + len] as char))
      {
        out.push(Diagnostic {
          code: "sql636",
          severity: Severity::Error,
          message: "`AUTOINCREMENT` is SQLite syntax -- PostgreSQL uses `GENERATED ALWAYS AS IDENTITY` (or serial)".into(),
          range: crate::range_at(start + i, start + i + len),
        });
        return;
      }
      i += 1;
    }
  }
}
