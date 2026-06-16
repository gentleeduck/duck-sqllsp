//! sql671: a `DESCRIBE t` / `DESC t` statement (MySQL / Oracle table
//! introspection). PostgreSQL has no such statement; inspect a table with the
//! psql meta-command `\d table`, or query `information_schema.columns` /
//! `pg_catalog`.
//!
//! Only the statement-leading form is flagged, so `ORDER BY x DESC` (the sort
//! direction) is never touched.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql671"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let u = upper.trim_start();
    let lead = upper.len() - u.len();
    let b = u.as_bytes();
    let kw = if b.starts_with(b"DESCRIBE") && b.get(8).is_none_or(|&c| !is_word(c as char)) {
      Some("DESCRIBE")
    } else if b.starts_with(b"DESC") && b.get(4).is_some_and(|&c| c.is_ascii_whitespace()) {
      Some("DESC")
    } else {
      None
    };
    if let Some(kw) = kw {
      out.push(Diagnostic {
        code: "sql671",
        severity: Severity::Error,
        message: format!("`{kw}` is MySQL/Oracle -- PostgreSQL has no DESCRIBE; use psql `\\d table` or query information_schema.columns"),
        range: crate::range_at(start + lead, start + lead + kw.len()),
      });
    }
  }
}
