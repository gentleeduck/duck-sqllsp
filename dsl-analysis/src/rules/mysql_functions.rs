//! sql596: MySQL-only functions that don't exist in PostgreSQL -- e.g.
//! `GROUP_CONCAT`, `DATE_FORMAT`, `STR_TO_DATE`, `UNIX_TIMESTAMP`. PG raises
//! 42883 ("function ... does not exist"). Each has a standard PG counterpart.
//! (NULL-coalesce functions like IFNULL/NVL are sql319's job.)

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// (function, PG equivalent).
const MYSQL_FNS: &[(&str, &str)] = &[
  ("group_concat(", "string_agg(...)"),
  ("date_format(", "to_char(...)"),
  ("str_to_date(", "to_date(...) / to_timestamp(...)"),
  ("unix_timestamp(", "extract(epoch from ...)"),
  ("from_unixtime(", "to_timestamp(...)"),
  ("curdate(", "current_date"),
  ("curtime(", "current_time"),
  ("rand(", "random()"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql596"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    for &(needle, pg) in MYSQL_FNS {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(needle) {
        let at = from + rel;
        from = at + needle.len();
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
          continue;
        }
        let name = needle.trim_end_matches('(');
        out.push(Diagnostic {
          code: "sql596",
          severity: Severity::Error,
          message: format!("`{name}` is a MySQL function with no PostgreSQL equivalent -- use `{pg}`"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
