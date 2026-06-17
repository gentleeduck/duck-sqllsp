//! sql747: `percentile_cont(1.5) WITHIN GROUP (...)` -- the percentile
//! fraction must be in [0, 1]. PostgreSQL raises 2202E ("percentile value ...
//! is not between 0 and 1") at runtime. Usually a percentage (50) written
//! where a fraction (0.5) was needed. (Companion to sql290 percentile_no_within.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql747"
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
      if !word_at(ub, i, b"PERCENTILE_CONT") && !word_at(ub, i, b"PERCENTILE_DISC") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 15);
      if ub.get(p) != Some(&b'(') {
        i += 15;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // Single numeric fraction (the ARRAY[...] multi-fraction form is skipped).
      if let Some((s, e)) = trim_range(ub, p + 1, close)
        && upper[s..e].parse::<f64>().is_ok_and(|v| !(0.0..=1.0).contains(&v))
      {
        out.push(Diagnostic {
          code: "sql747",
          severity: Severity::Warning,
          message: "percentile fraction must be between 0 and 1 -- raises an error at runtime (PG 2202E)".into(),
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
