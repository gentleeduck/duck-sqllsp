//! sql687: `COALESCE('x', ...)` -- the first argument is a non-NULL constant
//! literal, so COALESCE always returns it and every later argument is dead
//! code. Almost always the operands are in the wrong order (the fallback
//! literal belongs last). (Companion to sql493 coalesce_not_null for NOT NULL
//! columns and sql417 coalesce_dead_arg for duplicate / NULL args.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql687"
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
      if !word_at(ub, i, b"COALESCE") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 8);
      if ub.get(p) != Some(&b'(') {
        i += 8;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let args = top_level_args(ub, p, close);
      if args.len() >= 2
        && let Some((s, e)) = trim_range(ub, args[0].0, args[0].1)
        && is_nonnull_literal(&upper[s..e])
      {
        out.push(Diagnostic {
          code: "sql687",
          severity: Severity::Warning,
          message: "COALESCE with a constant first argument always returns it -- the later arguments are dead".into(),
          range: crate::range_at(start + s, start + e),
        });
      }
      i = close + 1;
    }
  }
}

/// True when `arg` is a self-contained non-NULL constant literal: a quoted
/// string, a number (optionally signed), or TRUE / FALSE.
fn is_nonnull_literal(arg: &str) -> bool {
  let b = arg.as_bytes();
  if b.is_empty() {
    return false;
  }
  if b[0] == b'\'' {
    // A single quoted string with nothing trailing it (no `||`, no cast).
    return b.len() >= 2 && b[b.len() - 1] == b'\'' && !b[1..b.len() - 1].contains(&b'\'');
  }
  if arg == "TRUE" || arg == "FALSE" {
    return true;
  }
  let digits = if b[0] == b'-' || b[0] == b'+' { &b[1..] } else { b };
  !digits.is_empty() && digits.iter().all(|&c| c.is_ascii_digit() || c == b'.')
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
