//! sql639: more cross-dialect string functions absent from PostgreSQL --
//! `HEX`, `UNHEX` (MySQL/SQLite), `SPACE` (MySQL/T-SQL), `QUOTE` (MySQL/SQLite).
//! PG raises 42883; each has a native counterpart. Complements sql622 / sql628.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[(&str, &str)] = &[
  ("hex(", "encode(x, 'hex')"),
  ("unhex(", "decode(x, 'hex')"),
  ("space(", "repeat(' ', n)"),
  ("quote(", "quote_literal(...) / quote_ident(...)"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql639"
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
          code: "sql639",
          severity: Severity::Error,
          message: format!("`{name}` is a non-PostgreSQL string function -- use `{pg}`"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
