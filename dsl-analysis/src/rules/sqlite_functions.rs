//! sql638: SQLite-only functions that don't exist in PostgreSQL -- `STRFTIME`,
//! `JULIANDAY`, `TYPEOF`, `PRINTF`, `LAST_INSERT_ROWID`. PG raises 42883; each
//! has a native counterpart. Complements sql596 / sql620 / sql622 / sql628 /
//! sql630 (other non-PG functions).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[(&str, &str)] = &[
  ("strftime(", "to_char(ts, 'FMT')"),
  ("julianday(", "EXTRACT(epoch FROM ts) / date arithmetic"),
  ("typeof(", "pg_typeof(...)"),
  ("printf(", "format(...)"),
  ("last_insert_rowid(", "lastval() / a RETURNING clause"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql638"
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
          code: "sql638",
          severity: Severity::Error,
          message: format!("`{name}` is a SQLite function with no PostgreSQL equivalent -- use `{pg}`"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
