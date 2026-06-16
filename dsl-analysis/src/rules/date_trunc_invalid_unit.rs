//! sql619: `date_trunc('<unit>', ...)` where `<unit>` is a string literal that
//! isn't one of PostgreSQL's recognised fields. At runtime PG raises 22023
//! ("unit \"...\" not recognized for type timestamp..."). Catches typos like
//! `'minutes'` (plural) or `'mon'` before they reach production.
//!
//! Only a literal first argument is checked; a column/parameter unit is left
//! alone.

use crate::clause_scan::split_top_level;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const VALID: &[&str] = &[
  "microseconds",
  "milliseconds",
  "second",
  "minute",
  "hour",
  "day",
  "week",
  "month",
  "quarter",
  "year",
  "decade",
  "century",
  "millennium",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql619"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("date_trunc(") {
      let at = from + rel;
      from = at + 11;
      if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
        continue;
      }
      // balance parens from the opening one
      let open = at + 10;
      let mut depth = 0i32;
      let mut j = open;
      let mut close = None;
      while j < n {
        match bytes[j] {
          b'(' => depth += 1,
          b')' => {
            depth -= 1;
            if depth == 0 {
              close = Some(j);
              break;
            }
          }
          _ => {}
        }
        j += 1;
      }
      let Some(close) = close else { continue };
      let args = &body[open + 1..close];
      let Some((first, off)) = split_top_level(args).into_iter().next() else {
        continue;
      };
      let t = first.trim();
      // only a single-quoted literal is statically checkable
      if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        let unit = t[1..t.len() - 1].trim().to_ascii_lowercase();
        if !VALID.contains(&unit.as_str()) {
          let lead = first.len() - first.trim_start().len();
          let abs = open + 1 + off + lead;
          out.push(Diagnostic {
            code: "sql619",
            severity: Severity::Error,
            message: format!("`{unit}` is not a valid date_trunc unit -- PG raises 22023; use one of second/minute/hour/day/week/month/quarter/year/etc"),
            range: crate::range_at(start + abs, start + abs + t.len()),
          });
        }
      }
    }
  }
}
