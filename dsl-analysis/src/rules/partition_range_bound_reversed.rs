//! sql758: `FOR VALUES FROM (x) TO (y)` where the lower partition bound
//! is not strictly less than the upper bound. PostgreSQL rejects an
//! empty partition range at CREATE/ALTER TABLE time ("empty range
//! bound specified for partition"). Only fires on a single-column
//! bound where both sides are literals of the same simple kind (both
//! numeric, or both single-quoted strings) -- multi-column and
//! unbounded (MINVALUE/MAXVALUE) bounds are left alone.

use crate::clause_scan::find_clause;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql758"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(fv) = find_clause(ub, b"FOR VALUES FROM") else { return };
    let i = skip_ws(ub, fv + "FOR VALUES FROM".len());
    if ub.get(i) != Some(&b'(') {
      return;
    }
    let Some(from_close) = match_paren(ub, i) else { return };
    let Some(from_arg) = single_arg(body, i, from_close) else { return };
    let mut j = skip_ws(ub, from_close + 1);
    if !ub[j..].starts_with(b"TO") || ub.get(j + 2).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
      return;
    }
    j = skip_ws(ub, j + 2);
    if ub.get(j) != Some(&b'(') {
      return;
    }
    let Some(to_close) = match_paren(ub, j) else { return };
    let Some(to_arg) = single_arg(body, j, to_close) else { return };
    let reversed = match (Literal::parse(from_arg), Literal::parse(to_arg)) {
      (Some(Literal::Num(x)), Some(Literal::Num(y))) => x >= y,
      (Some(Literal::Str(x)), Some(Literal::Str(y))) => x >= y,
      _ => false,
    };
    if reversed {
      out.push(Diagnostic {
        code: "sql758",
        severity: Severity::Error,
        message: "partition lower bound is not less than the upper bound -- PostgreSQL rejects an empty partition range".into(),
        range: crate::range_at(start + fv, start + to_close + 1),
      });
    }
  }
}

enum Literal<'a> {
  Num(f64),
  Str(&'a str),
}

impl<'a> Literal<'a> {
  fn parse(s: &'a str) -> Option<Literal<'a>> {
    let t = s.trim();
    if t.is_empty() {
      return None;
    }
    if let Ok(n) = t.parse::<f64>() {
      return Some(Literal::Num(n));
    }
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
      return Some(Literal::Str(&t[1..t.len() - 1]));
    }
    None
  }
}

/// The single top-level argument between `open+1` and `close`, or
/// `None` if there's a top-level comma (multi-column bound -- skip).
fn single_arg(body: &str, open: usize, close: usize) -> Option<&str> {
  let inner = body.as_bytes();
  let mut depth = 0i32;
  let mut k = open + 1;
  while k < close {
    match inner[k] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        k += 1;
        while k < close && inner[k] != b'\'' {
          k += 1;
        }
      },
      b',' if depth == 0 => return None,
      _ => {},
    }
    k += 1;
  }
  Some(&body[open + 1..close])
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
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
