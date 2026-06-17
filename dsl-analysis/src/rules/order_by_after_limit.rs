//! sql657: an `ORDER BY` that appears after `LIMIT` / `OFFSET` / `FETCH` at the
//! top level of a query. SQL fixes the clause order as
//! `... ORDER BY ... LIMIT ... OFFSET ...`, so `LIMIT 5 ORDER BY x` is a syntax
//! error (42601). The author almost certainly meant to order *then* limit.
//! Depth-0 only, so an inner subquery's ORDER BY before the outer LIMIT is fine.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

fn kw(b: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= b.len()
    && &b[i..i + w.len()] == w
    && (i == 0 || !is_word(b[i - 1] as char))
    && b.get(i + w.len()).is_none_or(|&c| !is_word(c as char))
}

/// Matches `ORDER` <ws> `BY` starting at `i`; returns true on match.
fn is_order_by(b: &[u8], i: usize) -> bool {
  if !kw(b, i, b"ORDER") {
    return false;
  }
  let mut j = i + 5;
  if j >= b.len() || !b[j].is_ascii_whitespace() {
    return false;
  }
  while j < b.len() && b[j].is_ascii_whitespace() {
    j += 1;
  }
  kw(b, j, b"BY")
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql657"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let b = upper.as_bytes();
    let n = b.len();
    let mut depth = 0i32;
    let mut limit_seen = false;
    let mut i = 0usize;
    while i < n {
      match b[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        _ if depth == 0 => {
          if kw(b, i, b"LIMIT") || kw(b, i, b"OFFSET") || kw(b, i, b"FETCH") {
            limit_seen = true;
          } else if is_order_by(b, i) {
            if limit_seen {
              out.push(Diagnostic {
                code: "sql657",
                severity: Severity::Error,
                message: "ORDER BY appears after LIMIT/OFFSET -- clause order must be ORDER BY then LIMIT (PG 42601)".into(),
                range: crate::range_at(start + i, start + i + 5),
              });
              return;
            }
            i += 5;
            continue;
          }
        }
        _ => {}
      }
      i += 1;
    }
  }
}
