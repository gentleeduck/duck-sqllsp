//! sql612: an aggregate function in a `RETURNING` list -- e.g.
//! `INSERT ... RETURNING count(*)`. PostgreSQL forbids set functions in
//! RETURNING and raises 42803 ("aggregate functions are not allowed in
//! RETURNING"). RETURNING yields one row per affected row, so there's nothing to
//! aggregate over; wrap the DML in a CTE and aggregate the result instead.
//!
//! Only aggregates at the RETURNING level (paren depth 0) are flagged, so an
//! aggregate inside a scalar subquery in the list is left alone.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const AGG: &[&str] = &[
  "COUNT", "SUM", "AVG", "MIN", "MAX", "STRING_AGG", "ARRAY_AGG", "JSON_AGG", "JSONB_AGG", "BOOL_AND",
  "BOOL_OR", "EVERY",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql612"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let ub = cleaned.to_ascii_uppercase();
    let b = ub.as_bytes();
    let n = b.len();
    // start scanning just after the depth-0 `RETURNING` keyword
    let Some(ret) = find_returning(b) else { return };
    let mut depth = 0i32;
    let mut i = ret;
    while i < n {
      match b[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        _ if depth == 0 => {
          for &agg in AGG {
            let l = agg.len();
            if i + l < n
              && &b[i..i + l] == agg.as_bytes()
              && (i == 0 || !is_word(b[i - 1] as char))
            {
              // require an opening paren (allowing whitespace) after the name
              let mut j = i + l;
              while j < n && b[j].is_ascii_whitespace() {
                j += 1;
              }
              if j < n && b[j] == b'(' {
                out.push(Diagnostic {
                  code: "sql612",
                  severity: Severity::Error,
                  message: "aggregate function in RETURNING -- PostgreSQL forbids this (42803); aggregate the DML result in a CTE instead".into(),
                  range: crate::range_at(start + i, start + i + l),
                });
                return;
              }
            }
          }
        }
        _ => {}
      }
      i += 1;
    }
  }
}

/// Byte index just past a depth-0 `RETURNING` keyword, if present.
fn find_returning(b: &[u8]) -> Option<usize> {
  let n = b.len();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i + 9 <= n {
    match b[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'R' if depth == 0 && &b[i..i + 9] == b"RETURNING" && (i == 0 || !is_word(b[i - 1] as char)) => {
        return Some(i + 9);
      }
      _ => {}
    }
    i += 1;
  }
  None
}
