//! sql553: `CREATE TABLE t (col int DEFAULT NULL)` -- a nullable column
//! already defaults to NULL, so `DEFAULT NULL` is redundant noise. (sql069
//! owns the contradictory `NOT NULL DEFAULT NULL`; this is the plain nullable
//! case.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql553"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let bytes = body.as_bytes();
    let n = ub.len();

    // Only inside a CREATE TABLE column list.
    if upper.find("CREATE TABLE").is_none() {
      return;
    }
    let Some(open) = ub.iter().position(|&b| b == b'(') else { return };
    let Some(close) = match_paren(ub, open) else { return };

    for (seg_s, seg_e) in split_top_level(open + 1, close, ub) {
      let seg = &upper[seg_s..seg_e];
      // `DEFAULT NULL` present, and the column is not NOT NULL (sql069's job).
      if let Some(rel) = find_default_null(seg.as_bytes())
        && !has_not_null(seg.as_bytes())
      {
        let at = seg_s + rel;
        // End of the `DEFAULT NULL` span (NULL keyword end).
        let mut e = at + "DEFAULT".len();
        e = skip_ws(bytes, e);
        e += 4; // NULL
        out.push(Diagnostic {
          code: "sql553",
          severity: Severity::Hint,
          message: "`DEFAULT NULL` is redundant -- a nullable column already defaults to NULL".into(),
          range: crate::range_at(start + at, start + e.min(n)),
        });
      }
    }
  }
}

/// Offset of a word-bounded `DEFAULT` followed (after whitespace) by `NULL`.
fn find_default_null(seg: &[u8]) -> Option<usize> {
  let mut i = 0usize;
  while i + 7 <= seg.len() {
    if &seg[i..i + 7] == b"DEFAULT"
      && (i == 0 || !is_word(seg[i - 1] as char))
      && seg.get(i + 7).is_some_and(|b| b.is_ascii_whitespace())
    {
      let mut j = i + 7;
      while j < seg.len() && seg[j].is_ascii_whitespace() {
        j += 1;
      }
      if j + 4 <= seg.len() && &seg[j..j + 4] == b"NULL" && (j + 4 == seg.len() || !is_word(seg[j + 4] as char)) {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}

fn has_not_null(seg: &[u8]) -> bool {
  let mut i = 0usize;
  while i + 8 <= seg.len() {
    if &seg[i..i + 8] == b"NOT NULL" && (i == 0 || !is_word(seg[i - 1] as char)) {
      return true;
    }
    i += 1;
  }
  false
}

fn split_top_level(from: usize, to: usize, ub: &[u8]) -> Vec<(usize, usize)> {
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut last = from;
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
      b',' if depth == 0 => {
        out.push((last, i));
        last = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  out.push((last, to));
  out
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
