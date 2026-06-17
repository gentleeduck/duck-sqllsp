//! sql647: `<col> IN (SELECT a, b, ... FROM ...)` where the subquery's SELECT
//! list has more than one column. An `IN` (or `= ANY`) subquery on a scalar
//! left-hand side must return exactly one column; PostgreSQL raises 42601
//! ("subquery must return only one column"). Either select a single column, or
//! use a row constructor on the left (`(a, b) IN (SELECT a, b ...)`).
//!
//! Conservative: only fires when the left-hand side is a bare column reference
//! (so row constructors and function-call LHS are never misread) and the SELECT
//! list has an explicit top-level comma.

use crate::clause_scan::{is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

fn matching(b: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < b.len() {
    match b[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      }
      _ => {}
    }
    i += 1;
  }
  None
}

/// Depth-0 byte index of a standalone `FROM` keyword in `b[lo..hi]`, else hi.
fn from_at(b: &[u8], lo: usize, hi: usize) -> usize {
  let mut depth = 0i32;
  let mut i = lo;
  while i + 4 <= hi {
    match b[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      _ if depth == 0
        && &b[i..i + 4] == b"FROM"
        && (i == lo || !is_word(b[i - 1] as char))
        && b.get(i + 4).is_none_or(|&c| !is_word(c as char)) =>
      {
        return i;
      }
      _ => {}
    }
    i += 1;
  }
  hi
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql647"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 2 <= n {
      if &ub[i..i + 2] == b"IN"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub.get(i + 2).is_some_and(|&b| b.is_ascii_whitespace())
      {
        // left-hand side must be a bare identifier
        let mut l = i;
        while l > 0 && ub[l - 1].is_ascii_whitespace() {
          l -= 1;
        }
        let lhs_ok = l > 0 && is_word(ub[l - 1] as char);
        // right-hand side: `( SELECT`
        let mut j = i + 2;
        while j < n && ub[j].is_ascii_whitespace() {
          j += 1;
        }
        if lhs_ok && j < n && ub[j] == b'(' {
          let open = j;
          let mut s = open + 1;
          while s < n && ub[s].is_ascii_whitespace() {
            s += 1;
          }
          if s + 6 <= n
            && &ub[s..s + 6] == b"SELECT"
            && ub.get(s + 6).is_none_or(|&b| !is_word(b as char))
            && let Some(close) = matching(ub, open)
          {
            {
              let sel = s + 6;
              let list_end = from_at(ub, sel, close);
              let list = &cleaned[sel..list_end];
              if split_top_level(list).len() > 1 {
                out.push(Diagnostic {
                  code: "sql647",
                  severity: Severity::Error,
                  message: "IN subquery returns more than one column -- PG raises 42601; select a single column or use a row constructor `(a, b) IN (...)`".into(),
                  range: crate::range_at(start + i, start + i + 2),
                });
              }
              i = close;
            }
          }
        }
      }
      i += 1;
    }
  }
}
