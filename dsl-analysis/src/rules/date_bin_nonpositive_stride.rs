//! sql737: `date_bin('0 seconds', ts, origin)` -- the stride (first argument)
//! must be a positive interval. PostgreSQL raises 22023 ("stride must be
//! greater than zero") at runtime for a zero or negative literal stride.
//! Usually a placeholder or a computed interval that collapsed to zero.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql737"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let bb = body.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      if !word_at(ub, i, b"DATE_BIN") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 8);
      if ub.get(p) != Some(&b'(') {
        i += 8;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // First argument: find the interval string literal within it.
      let first = top_level_args(ub, p, close).into_iter().next();
      if let Some((as_, ae)) = first
        && let Some((qs, qe)) = first_string(bb, as_, ae)
      {
        let content = &body[qs + 1..qe];
        if is_nonpositive_interval(content) {
          out.push(Diagnostic {
            code: "sql737",
            severity: Severity::Warning,
            message: "date_bin() stride must be greater than zero -- raises an error at runtime (PG 22023)".into(),
            range: crate::range_at(start + qs, start + (qe + 1).min(n)),
          });
        }
      }
      i = close + 1;
    }
  }
}

/// A zero (`'0 seconds'`, `'00:00:00'`) or negative (`'-1 day'`) interval text.
fn is_nonpositive_interval(content: &str) -> bool {
  let t = content.trim();
  if t.starts_with('-') {
    return true;
  }
  let digits: Vec<u8> = t.bytes().filter(u8::is_ascii_digit).collect();
  !digits.is_empty() && digits.iter().all(|&c| c == b'0')
}

/// First single-quoted string within `from..to`, as (open_quote, close_quote).
fn first_string(bb: &[u8], from: usize, to: usize) -> Option<(usize, usize)> {
  let mut i = from;
  while i < to {
    if bb[i] == b'\'' {
      let mut j = i + 1;
      while j < to && bb[j] != b'\'' {
        j += 1;
      }
      return (j < to).then_some((i, j));
    }
    i += 1;
  }
  None
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
