//! sql744: `array_position(arr, x) = 0` -- array_position returns a 1-based
//! index, or NULL when the element is absent. It is never 0 and never
//! negative, so `= 0`, `< 1`, `<= 0` and comparisons to negatives never match
//! (and NULL never matches anything). A frequent bug: code expecting a 0-based
//! index or a -1 "not found" sentinel. Test with `... IS NULL` /
//! `... IS NOT NULL` instead.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql744"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let n = bytes.len();
    let needle = "array_position(";

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
      let mut p = skip_ws(bytes, close + 1);
      let Some((op, after)) = read_op(bytes, p) else { continue };
      p = skip_ws(bytes, after);
      let Some((v, end)) = read_int(bytes, p, n) else { continue };
      // result is NULL or a 1-based index (>= 1).
      let never = match op {
        "=" => v < 1,
        "<" => v <= 1,
        "<=" => v < 1,
        _ => false,
      };
      if never {
        out.push(Diagnostic {
          code: "sql744",
          severity: Severity::Warning,
          message: format!("array_position() is NULL or a 1-based index, so `{op} {v}` never matches -- use IS NULL to test for absence"),
          range: crate::range_at(start + at, start + end),
        });
      }
    }
  }
}

fn read_op(bytes: &[u8], i: usize) -> Option<(&'static str, usize)> {
  match (bytes.get(i), bytes.get(i + 1)) {
    (Some(b'<'), Some(b'=')) => Some(("<=", i + 2)),
    (Some(b'<'), Some(b'>')) => None,
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
