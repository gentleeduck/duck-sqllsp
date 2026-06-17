//! sql690: `sqrt(-1)` -- the square root of a negative literal. PostgreSQL
//! raises 2201F ("cannot take square root of a negative number") at runtime.
//! Almost always a sign typo or a placeholder. (Companion to sql443
//! substring_negative_length for other negative-literal argument bugs.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql690"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      if !word_at(ub, i, b"SQRT") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 4);
      if ub.get(p) != Some(&b'(') {
        i += 4;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // Single argument that is a negative numeric literal: `-` then digits.
      if let Some((s, e)) = trim_range(ub, p + 1, close)
        && is_negative_number(&upper[s..e])
      {
        out.push(Diagnostic {
          code: "sql690",
          severity: Severity::Warning,
          message: "sqrt() of a negative number raises an error at runtime (PG 2201F)".into(),
          range: crate::range_at(start + s, start + e),
        });
      }
      i = close + 1;
    }
  }
}

fn is_negative_number(arg: &str) -> bool {
  let b = arg.as_bytes();
  b.len() >= 2 && b[0] == b'-' && b[1..].iter().all(|&c| c.is_ascii_digit() || c == b'.') && b[1..].iter().any(u8::is_ascii_digit)
}

fn trim_range(ub: &[u8], mut s: usize, mut e: usize) -> Option<(usize, usize)> {
  while s < e && ub[s].is_ascii_whitespace() {
    s += 1;
  }
  while e > s && ub[e - 1].is_ascii_whitespace() {
    e -= 1;
  }
  (s < e).then_some((s, e))
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
