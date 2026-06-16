//! sql643: Oracle scalar functions absent from PostgreSQL -- `ADD_MONTHS`,
//! `MONTHS_BETWEEN`, `NEXT_DAY`, `SYS_GUID`, `BITAND`. PG raises 42883; each has
//! a native counterpart (interval arithmetic, `age()`, `gen_random_uuid()`, the
//! `&` operator). Complements the Oracle DECODE / ROWNUM / DUAL lints.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[(&str, &str)] = &[
  ("add_months(", "d + interval 'n months'"),
  ("months_between(", "extract(... from age(a, b))"),
  ("next_day(", "date arithmetic on the target weekday"),
  ("sys_guid(", "gen_random_uuid()"),
  ("bitand(", "the `&` operator (a & b)"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql643"
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
          code: "sql643",
          severity: Severity::Error,
          message: format!("`{name}` is an Oracle function with no PostgreSQL equivalent -- use `{pg}`"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
