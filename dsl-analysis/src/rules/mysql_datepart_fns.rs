//! sql640: MySQL date-part functions with no PostgreSQL equivalent --
//! `DAYOFWEEK`, `DAYOFMONTH`, `DAYOFYEAR`, `WEEKDAY`, `WEEKOFYEAR`, `MONTHNAME`,
//! `DAYNAME`, `QUARTER`. PG raises 42883; extract the field with
//! `EXTRACT(field FROM ts)` / `date_part(...)`, and format names with
//! `to_char(ts, 'Month')` / `to_char(ts, 'Day')`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[(&str, &str)] = &[
  ("dayofweek(", "EXTRACT(dow FROM ts)"),
  ("dayofmonth(", "EXTRACT(day FROM ts)"),
  ("dayofyear(", "EXTRACT(doy FROM ts)"),
  ("weekofyear(", "EXTRACT(week FROM ts)"),
  ("weekday(", "EXTRACT(isodow FROM ts)"),
  ("monthname(", "to_char(ts, 'Month')"),
  ("dayname(", "to_char(ts, 'Day')"),
  ("quarter(", "EXTRACT(quarter FROM ts)"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql640"
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
          code: "sql640",
          severity: Severity::Error,
          message: format!("`{name}` is a MySQL date function with no PostgreSQL equivalent -- use `{pg}`"),
          range: crate::range_at(start + at, start + at + name.len()),
        });
      }
    }
  }
}
