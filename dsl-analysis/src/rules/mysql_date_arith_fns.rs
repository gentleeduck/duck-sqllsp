//! sql644: MySQL date-arithmetic functions absent from PostgreSQL -- `ADDDATE`,
//! `SUBDATE`, `ADDTIME`, `SUBTIME`, `LAST_DAY`, `SEC_TO_TIME`, `TIME_TO_SEC`,
//! `MAKEDATE`, `MAKETIME`. PG raises 42883; do the math with `interval`
//! arithmetic, `date_trunc`, `extract`, and `make_date` / `make_time`
//! (note the underscores -- those *are* PG functions). Complements sql640.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[(&str, &str)] = &[
  ("adddate(", "d + interval '...'"),
  ("subdate(", "d - interval '...'"),
  ("addtime(", "ts + interval '...'"),
  ("subtime(", "ts - interval '...'"),
  ("last_day(", "date_trunc('month', d) + interval '1 month' - interval '1 day'"),
  ("sec_to_time(", "n * interval '1 second'"),
  ("time_to_sec(", "extract(epoch from t)"),
  ("makedate(", "make_date(...) (PG, with underscore)"),
  ("maketime(", "make_time(...) (PG, with underscore)"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql644"
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
          code: "sql644",
          severity: Severity::Error,
          message: format!("`{name}` is a MySQL date function with no PostgreSQL equivalent -- use `{pg}`"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
