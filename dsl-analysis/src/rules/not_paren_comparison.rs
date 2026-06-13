//! sql537: `NOT (a = b)` -- negating a single comparison is clearer written
//! with the negated operator: `a <> b`. Likewise `NOT (a < b)` -> `a >= b`,
//! etc. Complements sql470 (which handles `NOT (col IN/LIKE/BETWEEN ...)`).
//! Only a lone comparison inside the parens is rewritten; anything with
//! AND/OR/IN/LIKE/BETWEEN/IS is left alone.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql537"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let bytes = body.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 3 <= n {
      if ub[i..i + 3] != *b"NOT" || (i > 0 && is_word(ub[i - 1] as char)) || is_word(*ub.get(i + 3).unwrap_or(&b' ') as char) {
        i += 1;
        continue;
      }
      let mut p = i + 3;
      while p < n && bytes[p].is_ascii_whitespace() {
        p += 1;
      }
      if bytes.get(p) != Some(&b'(') {
        i += 3;
        continue;
      }
      let Some(close) = match_paren(bytes, p) else { break };
      let inner = body[p + 1..close].trim();
      let inner_u = upper[p + 1..close].to_string();
      if let Some((op, neg, lhs, rhs)) = lone_comparison(inner, &inner_u) {
        out.push(Diagnostic {
          code: "sql537",
          severity: Severity::Hint,
          message: format!("`NOT ({lhs} {op} {rhs})` -- write `{lhs} {neg} {rhs}`"),
          range: crate::range_at(start + i, start + close + 1),
        });
      }
      i = close + 1;
    }
  }
}

/// If `inner` is exactly `lhs <cmp> rhs` (no AND/OR/IN/LIKE/BETWEEN/IS),
/// return (operator, negated operator, lhs, rhs).
fn lone_comparison<'a>(inner: &'a str, inner_u: &str) -> Option<(&'static str, &'static str, &'a str, &'a str)> {
  for kw in [" AND ", " OR ", " IN ", " IN(", " LIKE ", " ILIKE ", " BETWEEN ", " IS ", " SIMILAR ", " ANY", " ALL"] {
    if inner_u.contains(kw) {
      return None;
    }
  }
  let bytes = inner.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ if depth == 0 => {
        let two = (bytes.get(i), bytes.get(i + 1));
        let (op, neg, len) = match two {
          (Some(b'<'), Some(b'>')) => ("<>", "=", 2),
          (Some(b'!'), Some(b'=')) => ("!=", "=", 2),
          (Some(b'<'), Some(b'=')) => ("<=", ">", 2),
          (Some(b'>'), Some(b'=')) => (">=", "<", 2),
          (Some(b'<'), _) => ("<", ">=", 1),
          (Some(b'>'), _) => (">", "<=", 1),
          (Some(b'='), _) => ("=", "<>", 1),
          _ => {
            i += 1;
            continue;
          },
        };
        let lhs = inner[..i].trim();
        let rhs = inner[i + len..].trim();
        if lhs.is_empty() || rhs.is_empty() {
          return None;
        }
        return Some((op, neg, lhs, rhs));
      },
      _ => {},
    }
    i += 1;
  }
  None
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
