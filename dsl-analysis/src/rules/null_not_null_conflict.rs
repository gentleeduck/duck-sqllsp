//! sql564: `CREATE TABLE t (a int NULL NOT NULL)` -- a column declared both
//! explicitly nullable (`NULL`) and `NOT NULL`. Postgres rejects the
//! contradiction with 42601 ("conflicting NULL/NOT NULL declarations").

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql564"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("CREATE TABLE") {
      return;
    }
    let ub = upper.as_bytes();
    let Some(open) = ub.iter().position(|&b| b == b'(') else { return };
    if upper[..open].contains(" AS ") {
      return;
    }
    let Some(close) = match_paren(ub, open) else { return };

    for (s, e) in split_top_level(open + 1, close, ub) {
      let seg = &ub[s..e];
      if has_not_null(seg) && has_standalone_null(seg) {
        let lead = seg.iter().take_while(|b| b.is_ascii_whitespace()).count();
        let tail = seg.iter().rev().take_while(|b| b.is_ascii_whitespace()).count();
        out.push(Diagnostic {
          code: "sql564",
          severity: Severity::Error,
          message: "conflicting NULL / NOT NULL declarations on this column (PG error 42601)".into(),
          range: crate::range_at(start + s + lead, start + e - tail),
        });
      }
    }
  }
}

fn has_not_null(seg: &[u8]) -> bool {
  find_word(seg, b"NOT NULL").is_some()
}

/// A `NULL` keyword acting as the nullability constraint -- i.e. not part of
/// `NOT NULL`, and not preceded by `DEFAULT` or `IS`.
fn has_standalone_null(seg: &[u8]) -> bool {
  let n = seg.len();
  let mut i = 0usize;
  while i + 4 <= n {
    if &seg[i..i + 4] == b"NULL" && (i == 0 || !is_word(seg[i - 1] as char)) && (i + 4 == n || !is_word(seg[i + 4] as char))
    {
      // Preceding word.
      let mut j = i;
      while j > 0 && seg[j - 1].is_ascii_whitespace() {
        j -= 1;
      }
      let end = j;
      while j > 0 && is_word(seg[j - 1] as char) {
        j -= 1;
      }
      let prev = &seg[j..end];
      if prev != b"NOT" && prev != b"DEFAULT" && prev != b"IS" {
        return true;
      }
    }
    i += 1;
  }
  false
}

fn find_word(seg: &[u8], kw: &[u8]) -> Option<usize> {
  let n = seg.len();
  let m = kw.len();
  let mut i = 0usize;
  while i + m <= n {
    if seg[i..i + m] == *kw
      && (i == 0 || !is_word(seg[i - 1] as char))
      && (i + m == n || !is_word(seg[i + m] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
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
