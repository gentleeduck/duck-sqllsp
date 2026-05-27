//! sql055: `WHERE (single condition)` -- the parens add noise.
//!
//! Catches the simple case: a single `WHERE ( expr )` where the body
//! has no top-level AND/OR. Multi-clause predicates obviously need
//! grouping; this rule only flags the single-condition case.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql055"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Select(_) | StatementKind::Update(_) | StatementKind::Delete(_)) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = body.as_bytes();
    let n = bytes.len();

    // Find every `WHERE`. Skip whitespace; if next char is `(`,
    // capture the parenthesised body and check for nested AND/OR.
    let mut i = 0;
    while i + 5 <= n {
      if upper.as_bytes()[i..i + 5].eq_ignore_ascii_case(b"WHERE") {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let next_ok = i + 5 == n || !is_word(bytes[i + 5] as char);
        if prev_ok && next_ok {
          let mut j = i + 5;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          if j < n
            && bytes[j] == b'('
            && let Some(close) = match_paren(bytes, j)
          {
            // After the close paren, the next non-ws
            // char must be `;` or end-of-clause keyword
            // (GROUP/ORDER/LIMIT/OFFSET/UNION/EXCEPT/INTERSECT/RETURNING).
            let mut k = close + 1;
            while k < n && bytes[k].is_ascii_whitespace() {
              k += 1;
            }
            let trailing_ok = k == n
              || bytes[k] == b';'
              || starts_with_ci(bytes, k, b"GROUP")
              || starts_with_ci(bytes, k, b"ORDER")
              || starts_with_ci(bytes, k, b"LIMIT")
              || starts_with_ci(bytes, k, b"OFFSET")
              || starts_with_ci(bytes, k, b"UNION")
              || starts_with_ci(bytes, k, b"EXCEPT")
              || starts_with_ci(bytes, k, b"INTERSECT")
              || starts_with_ci(bytes, k, b"RETURNING")
              || starts_with_ci(bytes, k, b"HAVING");
            if !trailing_ok {
              i = j;
              continue;
            }
            let inner = &body[j + 1..close];
            if !has_top_level_and_or(inner) {
              out.push(Diagnostic {
                code: "sql055",
                severity: Severity::Hint,
                message: "redundant parens around single WHERE condition".into(),
                range: stmt.range,
              });
              return;
            }
          }
        }
      }
      i += 1;
    }
  }
}

fn starts_with_ci(bytes: &[u8], at: usize, needle: &[u8]) -> bool {
  at + needle.len() <= bytes.len() && bytes[at..at + needle.len()].eq_ignore_ascii_case(needle)
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = open;
  while i < n {
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
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn has_top_level_and_or(s: &str) -> bool {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = 0;
  while i < n {
    let c = bytes[i];
    match c {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    if depth == 0 && (c.is_ascii_alphabetic() || c == b'_') {
      let start = i;
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      let word = &s[start..i];
      if word.eq_ignore_ascii_case("AND") || word.eq_ignore_ascii_case("OR") {
        return true;
      }
      continue;
    }
    i += 1;
  }
  false
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
