//! sql622: MySQL-only string functions that don't exist in PostgreSQL --
//! `LCASE`, `UCASE`, `SUBSTRING_INDEX`, `FIND_IN_SET`. PG raises 42883; each has
//! a standard PG counterpart. Complements sql596 (GROUP_CONCAT/DATE_FORMAT/...)
//! and sql620 (DATEDIFF/...).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[(&str, &str)] = &[
  ("lcase(", "lower(...)"),
  ("ucase(", "upper(...)"),
  ("substring_index(", "split_part(...)"),
  ("find_in_set(", "array_position(string_to_array(...), ...) or `= ANY(...)`"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql622"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    for &(needle, pg) in FNS {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(needle) {
        let at = from + rel;
        from = at + needle.len();
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
          continue;
        }
        let name = needle.trim_end_matches('(');
        out.push(Diagnostic {
          code: "sql622",
          severity: Severity::Error,
          message: format!("`{name}` is a MySQL function with no PostgreSQL equivalent -- use `{pg}`"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
