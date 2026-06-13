//! sql541: a boolean literal operand that forces the whole condition to a
//! constant -- `... OR TRUE` (always true, matches everything) or `... AND
//! FALSE` (always false, matches nothing). Both are almost always a debugging
//! placeholder that escaped review and silently changes which rows are
//! affected. (The harmless `AND TRUE` / `OR FALSE` no-ops are left to sql282.)
//!
//! Precedence-aware: `OR TRUE` dominates regardless (OR binds loosest), but
//! `AND FALSE` only forces the result false when there is no top-level `OR`.
//! A literal that is the side of a comparison (`col = TRUE`) is never
//! mistaken for a standalone operand.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const STOPWORDS: &[&str] =
  &["GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT", "FETCH", "FOR"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql541"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();

    for needle in [&b"WHERE"[..], &b"ON"[..], &b"HAVING"[..]] {
      let mut from = 0usize;
      while let Some(rel) = find_clause(&ub[from..], needle).map(|p| p + from) {
        let ps = rel + needle.len();
        let pe = find_clause_end(ub, ps, STOPWORDS);
        scan(ub, start, ps, pe, out);
        from = pe.max(ps);
      }
    }
  }
}

fn scan(ub: &[u8], abs: usize, from: usize, to: usize, out: &mut Vec<Diagnostic>) {
  let has_or = has_top_level_or(ub, from, to);
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
      b'T' | b'F' if depth == 0 => {
        if let Some((lit, end)) = bool_literal(ub, i, to) {
          let lk = left_keyword(ub, from, i);
          let rk = right_keyword(ub, end, to);
          // Standalone only when both sides are logical boundaries.
          if lk != Kw::Other && rk != Kw::Other {
            let adj_or = lk == Kw::Or || rk == Kw::Or;
            let adj_and = lk == Kw::And || rk == Kw::And;
            let msg = if lit && adj_or {
              Some("`OR TRUE` here forces the whole condition true -- it matches every row")
            } else if !lit && adj_and && !has_or {
              Some("`AND FALSE` here forces the whole condition false -- it matches no rows")
            } else {
              None
            };
            if let Some(m) = msg {
              out.push(Diagnostic {
                code: "sql541",
                severity: Severity::Warning,
                message: m.into(),
                range: crate::range_at(abs + i, abs + end),
              });
            }
          }
          i = end;
          continue;
        }
      },
      _ => {},
    }
    i += 1;
  }
}

/// Match a word-bounded `TRUE` / `FALSE` at `i`. Returns (is_true, end).
fn bool_literal(ub: &[u8], i: usize, to: usize) -> Option<(bool, usize)> {
  for (kw, val) in [(&b"TRUE"[..], true), (&b"FALSE"[..], false)] {
    let end = i + kw.len();
    if end <= to && ub[i..end] == *kw && (i == 0 || !is_word(ub[i - 1] as char)) && (end >= to || !is_word(ub[end] as char))
    {
      return Some((val, end));
    }
  }
  None
}

#[derive(PartialEq)]
enum Kw {
  And,
  Or,
  Boundary, // `(` or clause start
  Other,    // comparison op / operand -> not a standalone operand
}

fn left_keyword(ub: &[u8], from: usize, at: usize) -> Kw {
  let mut j = at;
  while j > from && ub[j - 1].is_ascii_whitespace() {
    j -= 1;
  }
  if j == from {
    return Kw::Boundary;
  }
  if ub[j - 1] == b'(' {
    return Kw::Boundary;
  }
  let end = j;
  while j > from && is_word(ub[j - 1] as char) {
    j -= 1;
  }
  match &ub[j..end] {
    b"AND" => Kw::And,
    b"OR" => Kw::Or,
    _ => Kw::Other,
  }
}

fn right_keyword(ub: &[u8], end: usize, to: usize) -> Kw {
  let mut j = end;
  while j < to && ub[j].is_ascii_whitespace() {
    j += 1;
  }
  if j >= to {
    return Kw::Boundary;
  }
  if ub[j] == b')' {
    return Kw::Boundary;
  }
  let s = j;
  while j < to && is_word(ub[j] as char) {
    j += 1;
  }
  match &ub[s..j] {
    b"AND" => Kw::And,
    b"OR" => Kw::Or,
    _ => Kw::Other,
  }
}

fn has_top_level_or(ub: &[u8], from: usize, to: usize) -> bool {
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
      b'O' if depth == 0
        && i + 2 <= to
        && &ub[i..i + 2] == b"OR"
        && (i == from || !is_word(ub[i - 1] as char))
        && (i + 2 >= to || !is_word(ub[i + 2] as char)) =>
      {
        return true;
      },
      _ => {},
    }
    i += 1;
  }
  false
}
