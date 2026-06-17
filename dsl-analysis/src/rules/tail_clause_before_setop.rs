//! sql663: an `ORDER BY` / `LIMIT` / `OFFSET` / `FETCH` clause that appears
//! before a set operation (`UNION` / `INTERSECT` / `EXCEPT`) at the top level,
//! without parentheses -- e.g. `SELECT a FROM t ORDER BY a UNION SELECT b`.
//! Those clauses apply to the whole set operation and must come *after* it (or
//! the individual branch must be parenthesised); otherwise PostgreSQL raises
//! 42601 ("syntax error at or near UNION"). Wrap the branch:
//! `(SELECT a FROM t ORDER BY a LIMIT n) UNION ...`.
//!
//! Complements sql268, which handles the *parenthesised* branch case.

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
    "sql663"
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
    let mut tail_seen = false;
    let mut i = 0usize;
    while i < n {
      match b[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        _ if depth == 0 => {
          if kw(b, i, b"UNION") || kw(b, i, b"INTERSECT") || kw(b, i, b"EXCEPT") {
            if tail_seen {
              out.push(Diagnostic {
                code: "sql663",
                severity: Severity::Error,
                message: "ORDER BY / LIMIT before a set operation must be parenthesised -- they apply to the whole UNION/INTERSECT/EXCEPT (PG 42601)".into(),
                range: crate::range_at(start + i, start + i + 5),
              });
              return;
            }
          } else if is_order_by(b, i) {
            tail_seen = true;
            i += 5;
            continue;
          } else if kw(b, i, b"LIMIT") || kw(b, i, b"OFFSET") || kw(b, i, b"FETCH") {
            tail_seen = true;
          }
        }
        _ => {}
      }
      i += 1;
    }
  }
}
