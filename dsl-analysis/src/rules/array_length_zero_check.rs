//! sql547: `WHERE array_length(arr, 1) = 0` -- a wrong empty-array test.
//! `array_length` returns NULL for an empty array (and `>= 1` otherwise), so
//! it is never 0; the predicate never matches. Use `cardinality(arr) = 0`,
//! `arr = '{}'`, or `array_length(arr, 1) IS NULL` instead.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql547"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let n = bytes.len();
    let needle = "array_length(";
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find(needle) {
      let at = from + rel;
      if at > 0 && is_word(bytes[at - 1] as char) {
        from = at + needle.len();
        continue;
      }
      let open = at + needle.len() - 1;
      let Some(close) = match_paren(bytes, open) else { break };
      from = close + 1;
      // `<op> <int>` after the call -- flag the cases that can never hold.
      let mut p = skip_ws(bytes, close + 1);
      let Some((op, after)) = read_op(bytes, p) else { continue };
      p = skip_ws(bytes, after);
      let Some((v, end)) = read_int(bytes, p, n) else { continue };
      // array_length is NULL or >= 1, so these comparisons never hold.
      let never = match op {
        "=" => v <= 0,
        "<" => v <= 1,
        "<=" => v <= 0,
        _ => false,
      };
      if never {
        out.push(Diagnostic {
          code: "sql547",
          severity: Severity::Warning,
          message: "`array_length` is NULL for an empty array (never 0) -- use `cardinality(arr) = 0` or `arr = '{}'`"
            .into(),
          range: crate::range_at(start + at, start + end),
        });
      }
    }
  }
}

fn read_op(bytes: &[u8], i: usize) -> Option<(&'static str, usize)> {
  match (bytes.get(i), bytes.get(i + 1)) {
    (Some(b'<'), Some(b'=')) => Some(("<=", i + 2)),
    (Some(b'<'), Some(b'>')) => None, // `<>` is fine (non-empty test)
    (Some(b'<'), _) => Some(("<", i + 1)),
    (Some(b'='), _) => Some(("=", i + 1)),
    _ => None,
  }
}

fn read_int(bytes: &[u8], start: usize, to: usize) -> Option<(i64, usize)> {
  let mut i = start;
  if bytes.get(i) == Some(&b'-') {
    i += 1;
  }
  let ds = i;
  while i < to && bytes[i].is_ascii_digit() {
    i += 1;
  }
  if i == ds {
    return None;
  }
  if matches!(bytes.get(i), Some(&b) if b == b'.' || is_word(b as char)) {
    return None;
  }
  let v: i64 = std::str::from_utf8(&bytes[start..i]).ok()?.parse().ok()?;
  Some((v, i))
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
