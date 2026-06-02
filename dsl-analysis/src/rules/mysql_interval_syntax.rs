//! sql276: `INTERVAL 1 DAY` style (no quotes) -- that's the MySQL
//! literal form. PG requires `INTERVAL '1 day'`. Catches the common
//! mistake of porting MySQL SQL verbatim.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const UNITS: &[&str] = &[
  "DAY",
  "DAYS",
  "HOUR",
  "HOURS",
  "MINUTE",
  "MINUTES",
  "SECOND",
  "SECONDS",
  "MONTH",
  "MONTHS",
  "YEAR",
  "YEARS",
  "WEEK",
  "WEEKS",
  "MILLISECOND",
  "MILLISECONDS",
  "MICROSECOND",
  "MICROSECONDS",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql276"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = upper.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("INTERVAL") {
      let at = from + rel;
      if at > 0 {
        let prev = bytes[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 8;
          continue;
        }
      }
      let after = at + "INTERVAL".len();
      let post = body[after..].trim_start();
      // PG form: INTERVAL '<text>'
      if post.starts_with('\'') {
        from = after;
        continue;
      }
      // Cast form: ... ::INTERVAL -- accept.
      if at >= 2 && &body[at - 2..at] == "::" {
        from = after;
        continue;
      }
      // Find next number then unit keyword.
      let bytes_post = post.as_bytes();
      let mut k = 0usize;
      // optional sign
      if k < bytes_post.len() && (bytes_post[k] == b'-' || bytes_post[k] == b'+') {
        k += 1
      }
      let num_start = k;
      while k < bytes_post.len() && (bytes_post[k].is_ascii_digit() || bytes_post[k] == b'.') {
        k += 1
      }
      if k == num_start {
        from = after;
        continue;
      }
      while k < bytes_post.len() && bytes_post[k].is_ascii_whitespace() {
        k += 1
      }
      let unit_start = k;
      while k < bytes_post.len() && bytes_post[k].is_ascii_alphabetic() {
        k += 1
      }
      let unit: String = post[unit_start..k].to_ascii_uppercase();
      if !UNITS.contains(&unit.as_str()) {
        from = after;
        continue;
      }
      let abs_s = start + at;
      let abs_e = start + after + (post.len() - bytes_post.len()) + k;
      out.push(Diagnostic {
        code: "sql276",
        severity: Severity::Error,
        message: "`INTERVAL` literal without quotes -- MySQL syntax; PG requires `INTERVAL '<n> <unit>'` like `INTERVAL '1 day'`".to_string(),
        range: crate::range_at(abs_s, abs_e),
      });
      from = after + k;
    }
  }
}
