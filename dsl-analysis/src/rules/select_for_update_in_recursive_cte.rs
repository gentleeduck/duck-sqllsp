//! sql132: `SELECT ... FOR UPDATE` inside the recursive arm of a CTE
//! is forbidden by PG -- the planner rejects it.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql132"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.contains("WITH RECURSIVE") {
      return;
    }
    // Look for `FOR UPDATE` or `FOR SHARE` between WITH RECURSIVE
    // and the matching closing paren of the CTE body.
    let Some(wr) = upper.find("WITH RECURSIVE") else { return };
    let after_wr = &body[wr + 14..];
    let bytes = after_wr.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    // Skip leading whitespace.
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    // CTE name (identifier).
    while i < n && is_word(bytes[i] as char) {
      i += 1;
    }
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    // Optional column list `(a, b)`.
    if i < n && bytes[i] == b'(' {
      let mut depth = 1i32;
      i += 1;
      while i < n && depth > 0 {
        match bytes[i] {
          b'(' => depth += 1,
          b')' => depth -= 1,
          _ => {},
        }
        i += 1;
      }
      while i < n && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
    }
    // `AS`.
    if i + 2 > n || !after_wr[i..i + 2].eq_ignore_ascii_case("AS") {
      return;
    }
    i += 2;
    while i < n && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    // Optional materialization marker.
    for kw in ["NOT MATERIALIZED", "MATERIALIZED"] {
      if i + kw.len() <= n && after_wr[i..i + kw.len()].eq_ignore_ascii_case(kw) {
        i += kw.len();
        while i < n && bytes[i].is_ascii_whitespace() {
          i += 1;
        }
        break;
      }
    }
    if i >= n || bytes[i] != b'(' {
      return;
    }
    let body_open = i + 1;
    let mut depth = 1i32;
    let mut j = body_open;
    while j < n && depth > 0 {
      match bytes[j] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b'\'' => {
          j += 1;
          while j < n && bytes[j] != b'\'' {
            j += 1;
          }
        },
        _ => {},
      }
      if depth == 0 {
        break;
      }
      j += 1;
    }
    if j >= n {
      return;
    }
    let cte_body = &after_wr[body_open..j];
    let cte_upper = cte_body.to_ascii_uppercase();
    let fu = cte_upper.find("FOR UPDATE").or_else(|| cte_upper.find("FOR SHARE"));
    let Some(fu_at) = fu else { return };
    let abs_start = start + wr + 14 + body_open + fu_at;
    let abs_end = abs_start + 10;
    out.push(Diagnostic {
      code: "sql132",
      severity: Severity::Error,
      message: "FOR UPDATE / FOR SHARE is not allowed inside the recursive arm of a CTE -- PG rejects the query".into(),
      range: crate::range_at(abs_start, abs_end),
    });
  }
}

