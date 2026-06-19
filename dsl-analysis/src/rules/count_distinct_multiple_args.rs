//! sql755: `count(DISTINCT a, b)` -- a single-argument aggregate given several
//! comma-separated DISTINCT expressions. count / sum / avg / min / max (etc.)
//! take exactly one argument, so PostgreSQL raises 42883 ("function ... does
//! not exist"). To count distinct combinations use `count(DISTINCT (a, b))`
//! (a row value) or `count(DISTINCT ROW(a, b))`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const SINGLE_ARG_AGGS: &[&[u8]] = &[
  b"COUNT", b"SUM", b"AVG", b"MIN", b"MAX", b"BOOL_AND", b"BOOL_OR", b"EVERY", b"BIT_AND", b"BIT_OR", b"STDDEV",
  b"VARIANCE", b"VAR_POP", b"VAR_SAMP", b"STDDEV_POP", b"STDDEV_SAMP",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql755"
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
      let Some(len) = SINGLE_ARG_AGGS.iter().copied().find(|a| word_at(ub, i, a)).map(<[u8]>::len) else {
        i += 1;
        continue;
      };
      let p = skip_ws(ub, i + len);
      if ub.get(p) != Some(&b'(') {
        i += len;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let d = skip_ws(ub, p + 1);
      if word_at(ub, d, b"DISTINCT")
        && let Some(comma) = top_level_comma(ub, d + 8, close)
      {
        out.push(Diagnostic {
          code: "sql755",
          severity: Severity::Error,
          message: "a single-argument aggregate can't take multiple DISTINCT expressions -- use count(DISTINCT (a, b))".into(),
          range: crate::range_at(start + comma, start + comma + 1),
        });
      }
      i = close + 1;
    }
  }
}

fn top_level_comma(ub: &[u8], from: usize, to: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = from;
  while i < to {
    match ub[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < to && ub[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => return Some(i),
      _ => {},
    }
    i += 1;
  }
  None
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
