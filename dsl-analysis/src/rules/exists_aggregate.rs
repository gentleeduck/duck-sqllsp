//! sql735: `EXISTS (SELECT count(*) FROM ...)` -- a subquery whose projection
//! is a bare aggregate (and that has no GROUP BY / HAVING) always returns
//! exactly one row, so the EXISTS is always true (and `NOT EXISTS` always
//! false). Almost always a misunderstanding: use `EXISTS (SELECT 1 FROM ...)`
//! to test for any rows, or compare the count directly. (Companion to sql441
//! uncorrelated_exists and sql201 exists_select_star.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const AGGS: &[&[u8]] = &[
  b"COUNT", b"SUM", b"AVG", b"MIN", b"MAX", b"BOOL_AND", b"BOOL_OR", b"EVERY", b"BIT_AND", b"BIT_OR", b"STRING_AGG",
  b"ARRAY_AGG", b"JSON_AGG", b"JSONB_AGG", b"STDDEV", b"VARIANCE",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql735"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 6 <= n {
      if !word_at(ub, i, b"EXISTS") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 6);
      if ub.get(p) != Some(&b'(') {
        i += 6;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // Inside the parens: SELECT <aggregate>( ... and no GROUP BY / HAVING.
      let q = skip_ws(ub, p + 1);
      if word_at(ub, q, b"SELECT") {
        let a = skip_ws(ub, q + 6);
        let is_agg = AGGS.iter().any(|f| word_at(ub, a, f) && ub.get(skip_ws(ub, a + f.len())) == Some(&b'('));
        if is_agg && !region_has(ub, p + 1, close, b"GROUP BY") && !region_has(ub, p + 1, close, b"HAVING") {
          out.push(Diagnostic {
            code: "sql735",
            severity: Severity::Warning,
            message: "EXISTS over a bare-aggregate subquery is always true -- the aggregate always returns one row".into(),
            range: crate::range_at(start + i, start + a),
          });
        }
      }
      i = close + 1;
    }
  }
}

/// Whether `needle` (already uppercase, may contain a space) appears
/// word-bounded between `from` and `to`.
fn region_has(ub: &[u8], from: usize, to: usize, needle: &[u8]) -> bool {
  let mut i = from;
  while i + needle.len() <= to {
    if &ub[i..i + needle.len()] == needle
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + needle.len() == ub.len() || !is_word(ub[i + needle.len()] as char))
    {
      return true;
    }
    i += 1;
  }
  false
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
