//! sql676: `COUNT(DISTINCT 1)` / `COUNT(DISTINCT 'x')` -- counting the
//! distinct values of a constant. A constant has exactly one distinct value,
//! so this returns 1 for any non-empty group (0 for an empty one) -- never
//! what was meant. Either `COUNT(*)` (rows) or `COUNT(DISTINCT col)` (a real
//! column) was almost certainly intended.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql676"
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
      if !word_at(ub, i, b"COUNT") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 5);
      if ub.get(p) != Some(&b'(') {
        i += 5;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let d = skip_ws(ub, p + 1);
      if word_at(ub, d, b"DISTINCT") {
        let a = skip_ws(ub, d + 8);
        // The single argument must be a bare literal: `<lit> )`.
        if let Some(lit_end) = literal_end(ub, a, close)
          && skip_ws(ub, lit_end) == close
        {
          out.push(Diagnostic {
            code: "sql676",
            severity: Severity::Warning,
            message: "COUNT(DISTINCT <constant>) is always 1 (or 0) -- count rows with COUNT(*) or a real column".into(),
            range: crate::range_at(start + a, start + lit_end),
          });
        }
      }
      i = close + 1;
    }
  }
}

/// If a literal starts at `i` (within `to`), return the byte just past it.
/// Recognises numbers, single-quoted strings, and NULL / TRUE / FALSE.
fn literal_end(ub: &[u8], i: usize, to: usize) -> Option<usize> {
  if i >= to {
    return None;
  }
  match ub[i] {
    b'\'' => {
      let mut j = i + 1;
      while j < to && ub[j] != b'\'' {
        j += 1;
      }
      (j < to).then_some(j + 1)
    },
    c if c.is_ascii_digit() => {
      let mut j = i + 1;
      while j < to && (ub[j].is_ascii_digit() || ub[j] == b'.') {
        j += 1;
      }
      Some(j)
    },
    _ => {
      for kw in [&b"NULL"[..], &b"TRUE"[..], &b"FALSE"[..]] {
        if word_at(ub, i, kw) {
          return Some(i + kw.len());
        }
      }
      None
    },
  }
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
