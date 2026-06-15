//! sql556: `col = ANY(ARRAY[1, 2, 3])` -- equivalent to `col IN (1, 2, 3)`,
//! the idiom most readers reach for first. (sql521 handles the single-element
//! case; sql548 the `<> ALL` -> `NOT IN` mirror; this is multi-element
//! `= ANY`.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql556"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 3 <= n {
      if ub[i..i + 3] != *b"ANY" || (i > 0 && is_word(ub[i - 1] as char)) {
        i += 1;
        continue;
      }
      // Preceding operator must be `=`.
      let mut b = i;
      while b > 0 && ub[b - 1].is_ascii_whitespace() {
        b -= 1;
      }
      let eq = b >= 1 && ub[b - 1] == b'=' && !(b >= 2 && matches!(ub[b - 2], b'<' | b'>' | b'!'));
      let mut p = skip_ws(ub, i + 3);
      if !eq || ub.get(p) != Some(&b'(') {
        i += 3;
        continue;
      }
      let Some(call_close) = match_pair(ub, p, b'(', b')') else { break };
      p = skip_ws(ub, p + 1);
      if !ub[p..].starts_with(b"ARRAY") {
        i = call_close + 1;
        continue;
      }
      p = skip_ws(ub, p + 5);
      if ub.get(p) != Some(&b'[') {
        i = call_close + 1;
        continue;
      }
      let Some(rb) = match_pair(ub, p, b'[', b']') else {
        i = call_close + 1;
        continue;
      };
      let inner = body[p + 1..rb].trim();
      if !inner.is_empty() && has_top_level_comma(inner) {
        out.push(Diagnostic {
          code: "sql556",
          severity: Severity::Hint,
          message: format!("`= ANY(ARRAY[{inner}])` is just `IN ({inner})` -- shorter and clearer"),
          range: crate::range_at(start + (b - 1), start + call_close + 1),
        });
      }
      i = call_close + 1;
    }
  }
}

fn has_top_level_comma(s: &str) -> bool {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b',' if depth == 0 => return true,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  false
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}

fn match_pair(bytes: &[u8], from: usize, open: u8, close: u8) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = from;
  while i < bytes.len() {
    let b = bytes[i];
    if b == open {
      depth += 1;
    } else if b == close {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
    } else if b == b'\'' {
      i += 1;
      while i < bytes.len() && bytes[i] != b'\'' {
        i += 1;
      }
    }
    i += 1;
  }
  None
}
