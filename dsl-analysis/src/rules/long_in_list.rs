//! sql074: `WHERE x IN (a, b, c, ...)` with > 50 items. Long IN-lists
//! defeat the planner; suggest a temp table or `= ANY(ARRAY[...])`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const THRESHOLD: usize = 50;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql074"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = body.as_bytes();
    let upper_bytes = upper.as_bytes();
    let n = bytes.len();

    let mut i = 0;
    while i + 4 <= n {
      if &upper[i..i + 4] == " IN " || (&upper[i..i + 3] == "IN " && i == 0) {
        let after_in = if i + 4 <= n && &upper[i..i + 4] == " IN " { i + 4 } else { i + 3 };
        let _ = upper_bytes;
        let mut j = after_in;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n
          && bytes[j] == b'('
          && let Some(close) = match_paren(bytes, j)
        {
          let inner = &body[j + 1..close];
          // Skip subqueries (start with SELECT).
          if inner.trim_start().to_ascii_uppercase().starts_with("SELECT") {
            i += 1;
            continue;
          }
          let count = top_level_commas(inner) + 1;
          if count > THRESHOLD {
            let abs_start = start + j;
            let abs_end = start + close + 1;
            out.push(Diagnostic {
                                code: "sql074",
                                severity: Severity::Hint,
                                message: format!(
                                    "IN-list with {count} items -- prefer `= ANY(ARRAY[...])` or load into a temp table for planner stability"
                                ),
                                range: crate::range_at(abs_start, abs_end),
                            });
            return;
          }
        }
      }
      i += 1;
    }
  }
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

fn top_level_commas(s: &str) -> usize {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut count = 0usize;
  let mut depth = 0i32;
  let mut i = 0;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => count += 1,
      _ => {},
    }
    i += 1;
  }
  count
}
