//! sql630: SQL Server (T-SQL) identity / GUID functions that don't exist in
//! PostgreSQL -- `NEWID`, `NEWSEQUENTIALID`, `SCOPE_IDENTITY`, `IDENT_CURRENT`.
//! PG raises 42883. Generate UUIDs with `gen_random_uuid()`, and read a freshly
//! inserted serial/identity value with `RETURNING`, `lastval()`, or
//! `currval('seq')`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[(&str, &str)] = &[
  ("newid(", "gen_random_uuid()"),
  ("newsequentialid(", "gen_random_uuid()"),
  ("scope_identity(", "RETURNING / lastval()"),
  ("ident_current(", "currval('seq') / lastval()"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql630"
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
          code: "sql630",
          severity: Severity::Error,
          message: format!("`{name}` is a SQL Server function with no PostgreSQL equivalent -- use `{pg}`"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
