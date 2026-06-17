//! sql732: `acosh(0)` / `atanh(1)` -- an argument outside the function's
//! domain. `acosh(x)` needs x >= 1 and `atanh(x)` needs -1 < x < 1;
//! PostgreSQL raises 2201E ("input is out of range") at runtime otherwise.
//! Usually a value that should have been clamped or normalised. (Companion to
//! sql694 acos_asin_domain.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql732"
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
      // acosh: x >= 1 required.  atanh: |x| < 1 required.
      let bad: fn(f64) -> bool = if word_at(ub, i, b"ACOSH") {
        |v| v < 1.0
      } else if word_at(ub, i, b"ATANH") {
        |v| !(-1.0..1.0).contains(&v)
      } else {
        i += 1;
        continue;
      };
      let p = skip_ws(ub, i + 5);
      if ub.get(p) != Some(&b'(') {
        i += 5;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      if let Some((s, e)) = trim_range(ub, p + 1, close)
        && upper[s..e].parse::<f64>().is_ok_and(bad)
      {
        out.push(Diagnostic {
          code: "sql732",
          severity: Severity::Warning,
          message: "acosh/atanh argument is outside its domain -- raises an error at runtime (PG 2201E)".into(),
          range: crate::range_at(start + s, start + e),
        });
      }
      i = close + 1;
    }
  }
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
