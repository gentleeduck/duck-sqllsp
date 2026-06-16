//! sql620: MySQL / SQL Server date arithmetic functions that don't exist in
//! PostgreSQL -- `DATEDIFF`, `DATEADD`, `TIMESTAMPDIFF`, `DATEPART`. PG raises
//! 42883 ("function ... does not exist"). PostgreSQL does date math with native
//! operators and EXTRACT instead. (Complements sql596 / the non-PG date fns
//! rule, which cover GETDATE/SYSDATE/DATE_FORMAT and friends.)

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// (function, PG guidance).
const FNS: &[(&str, &str)] = &[
  ("datediff(", "subtract dates directly (`a - b`) or use EXTRACT/AGE"),
  ("dateadd(", "add an interval: `ts + interval '1 day'`"),
  ("timestampdiff(", "`EXTRACT(epoch FROM (a - b))` (then scale to the unit)"),
  ("datepart(", "`EXTRACT(field FROM ts)` / `date_part('field', ts)`"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql620"
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
          code: "sql620",
          severity: Severity::Error,
          message: format!("`{name}` is a MySQL/SQL Server function with no PostgreSQL equivalent -- {pg}"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
