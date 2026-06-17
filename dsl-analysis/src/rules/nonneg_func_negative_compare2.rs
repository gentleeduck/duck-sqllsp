//! sql738: comparing a never-negative function against a negative value (or
//! `< 0`), so the predicate never matches -- e.g. `WHERE strpos(s, x) = -1` or
//! `array_length(a, 1) < 0`. Covers position / strpos / array_length /
//! jsonb_array_length / ascii. A frequent cross-language bug: these return 0
//! (or NULL), never -1, for "not found" / "empty". (Extends sql552, which owns
//! abs / length-family / cardinality; sql547 owns `array_length(...) = 0`.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const FNS: &[&str] = &["position(", "strpos(", "array_length(", "jsonb_array_length(", "ascii("];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql738"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let n = bytes.len();
    for fname in FNS {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(fname) {
        let at = from + rel;
        if at > 0 && is_word(bytes[at - 1] as char) {
          from = at + fname.len();
          continue;
        }
        let open = at + fname.len() - 1;
        let Some(close) = match_paren(bytes, open) else { break };
        from = close + 1;
        let mut p = skip_ws(bytes, close + 1);
        let Some((op, after)) = read_op(bytes, p) else { continue };
        p = skip_ws(bytes, after);
        let Some((v, end)) = read_int(bytes, p, n) else { continue };
        let never = match op {
          "=" => v < 0,
          "<" => v <= 0,
          "<=" => v < 0,
          _ => false,
        };
        if never {
          let name = fname.trim_end_matches('(');
          out.push(Diagnostic {
            code: "sql738",
            severity: Severity::Warning,
            message: format!("`{name}(...)` is never negative, so `{op} {v}` never matches"),
            range: crate::range_at(start + at, start + end),
          });
        }
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
