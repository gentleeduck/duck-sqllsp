//! sql749: `daterange('2024-01-01', '2023-01-01')` -- the lower date/timestamp
//! bound is later than the upper one. PostgreSQL raises 22000 ("range lower
//! bound must be less than or equal to range upper bound") at runtime. Usually
//! transposed arguments. Fires only when both bounds are ISO date/timestamp
//! string literals. (Companion to sql746 range_lower_gt_upper for numeric ranges.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const CTORS: &[&[u8]] = &[b"DATERANGE", b"TSRANGE", b"TSTZRANGE"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql749"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      let Some(clen) = CTORS.iter().copied().find(|c| word_at(ub, i, c)).map(<[u8]>::len) else {
        i += 1;
        continue;
      };
      let p = skip_ws(ub, i + clen);
      if ub.get(p) != Some(&b'(') {
        i += clen;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let args = top_level_args(ub, p, close);
      if args.len() >= 2
        && let (Some(lo), Some(hi)) = (iso_literal(ub, body, args[0]), iso_literal(ub, body, args[1]))
        && lo > hi
      {
        out.push(Diagnostic {
          code: "sql749",
          severity: Severity::Warning,
          message: "range lower bound is later than the upper bound -- raises an error at runtime (PG 22000)".into(),
          range: crate::range_at(start + i, start + close + 1),
        });
      }
      i = close + 1;
    }
  }
}

/// The content of an ISO date/timestamp string literal (`'YYYY-MM-DD...'`).
fn iso_literal(ub: &[u8], body: &str, arg: (usize, usize)) -> Option<String> {
  let (s, e) = trim_range(ub, arg.0, arg.1)?;
  if e < s + 12 || ub[s] != b'\'' || ub[e - 1] != b'\'' {
    return None;
  }
  let c = &body[s + 1..e - 1];
  let b = c.as_bytes();
  // YYYY-MM-DD prefix.
  if b.len() >= 10
    && b[0..4].iter().all(u8::is_ascii_digit)
    && b[4] == b'-'
    && b[5..7].iter().all(u8::is_ascii_digit)
    && b[7] == b'-'
    && b[8..10].iter().all(u8::is_ascii_digit)
  {
    Some(c.to_string())
  } else {
    None
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

fn top_level_args(ub: &[u8], open: usize, close: usize) -> Vec<(usize, usize)> {
  let mut args = Vec::new();
  let mut depth = 0i32;
  let mut argstart = open + 1;
  let mut i = open + 1;
  while i < close {
    match ub[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < close && ub[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => {
        args.push((argstart, i));
        argstart = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  if argstart < close || !args.is_empty() {
    args.push((argstart, close));
  }
  args
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
