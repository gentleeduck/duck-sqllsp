//! sql695: an aggregate call nested directly inside another aggregate, e.g.
//! `sum(count(*))` or `max(avg(x))`. PostgreSQL raises 42803 ("aggregate
//! function calls cannot be nested"). The usual fix is a subquery (aggregate
//! the inner result one query level down) or a window function. A nested
//! aggregate that *is* inside a subquery argument is fine and not flagged.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const AGGS: &[&[u8]] = &[
  b"COUNT", b"SUM", b"AVG", b"MIN", b"MAX", b"BOOL_AND", b"BOOL_OR", b"EVERY", b"BIT_AND", b"BIT_OR", b"STRING_AGG",
  b"ARRAY_AGG", b"JSON_AGG", b"JSONB_AGG", b"STDDEV", b"VARIANCE", b"VAR_POP", b"VAR_SAMP", b"STDDEV_POP", b"STDDEV_SAMP",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql695"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      let Some(len) = agg_at(ub, i) else {
        i += 1;
        continue;
      };
      let p = skip_ws(ub, i + len);
      if ub.get(p) != Some(&b'(') {
        i += len;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      if let Some((s, e)) = inner_agg(ub, p + 1, close) {
        out.push(Diagnostic {
          code: "sql695",
          severity: Severity::Error,
          message: "aggregate calls cannot be nested (PG 42803) -- use a subquery or window function".into(),
          range: crate::range_at(start + s, start + e),
        });
      }
      i += len;
    }
  }
}

/// First aggregate call between `from` and `to` that is *not* inside a nested
/// subquery (a parenthesised `SELECT` / `WITH`).
fn inner_agg(ub: &[u8], from: usize, to: usize) -> Option<(usize, usize)> {
  let mut stack: Vec<bool> = Vec::new(); // true = inside a subquery scope
  let mut i = from;
  while i < to {
    match ub[i] {
      b'\'' => {
        i += 1;
        while i < to && ub[i] != b'\'' {
          i += 1;
        }
      },
      b'(' => {
        let j = skip_ws(ub, i + 1);
        let is_sub = word_at(ub, j, b"SELECT") || word_at(ub, j, b"WITH");
        stack.push(is_sub);
      },
      b')' => {
        stack.pop();
      },
      _ => {
        if !stack.iter().any(|&s| s)
          && let Some(len) = agg_at(ub, i)
          && ub.get(skip_ws(ub, i + len)) == Some(&b'(')
        {
          return Some((i, i + len));
        }
      },
    }
    i += 1;
  }
  None
}

fn agg_at(ub: &[u8], i: usize) -> Option<usize> {
  AGGS.iter().copied().find(|a| word_at(ub, i, a)).map(<[u8]>::len)
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
