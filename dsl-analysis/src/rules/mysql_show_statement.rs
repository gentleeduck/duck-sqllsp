//! sql670: a MySQL `SHOW TABLES` / `SHOW DATABASES` / `SHOW COLUMNS` / `SHOW
//! CREATE TABLE` / ... introspection statement. PostgreSQL's `SHOW` only
//! displays configuration parameters (`SHOW search_path`, `SHOW ALL`); to list
//! schema objects use the information_schema / pg_catalog views or psql
//! meta-commands (`\dt`, `\d table`, `\l`, `\di`).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// MySQL-only words that can follow `SHOW`.
const MYSQL_SHOW: &[&str] = &[
  "TABLES", "DATABASES", "SCHEMAS", "COLUMNS", "INDEX", "INDEXES", "KEYS", "GRANTS", "TRIGGERS",
  "PROCESSLIST", "ENGINES", "VARIABLES", "WARNINGS", "ERRORS", "CREATE", "FULL", "TABLE", "PROCEDURE",
  "FUNCTION", "EVENTS",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql670"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let u = upper.trim_start();
    if !u.starts_with("SHOW") {
      return;
    }
    let b = upper.as_bytes();
    let n = b.len();
    let lead = upper.len() - u.len();
    let mut j = lead + 4;
    while j < n && b[j].is_ascii_whitespace() {
      j += 1;
    }
    for &w in MYSQL_SHOW {
      let l = w.len();
      if j + l <= n && &b[j..j + l] == w.as_bytes() && b.get(j + l).is_none_or(|&c| !is_word(c as char)) {
        out.push(Diagnostic {
          code: "sql670",
          severity: Severity::Error,
          message: format!("`SHOW {w}` is MySQL -- PostgreSQL's SHOW only reads config; use information_schema / pg_catalog or psql `\\dt` / `\\d`"),
          range: crate::range_at(start + lead, start + j + l),
        });
        return;
      }
    }
  }
}
