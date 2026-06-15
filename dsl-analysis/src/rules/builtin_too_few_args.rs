//! sql567: common built-in functions called with too few arguments -- e.g.
//! `to_char(x)` (needs a format), `lpad(s)` (needs a length), `split_part(s,
//! d)` (needs a field index). The single/short forms don't exist, so Postgres
//! raises 42883 ("function ... does not exist"). These built-ins aren't in the
//! catalog, so sql513's signature check doesn't see them.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// (function, minimum required arguments).
const MIN_ARGS: &[(&str, usize)] = &[
  ("to_char", 2),
  ("to_date", 2),
  ("to_number", 2),
  ("date_trunc", 2),
  ("lpad", 2),
  ("rpad", 2),
  ("split_part", 3),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql567"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    for &(fname, min) in MIN_ARGS {
      let needle = format!("{fname}(");
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(&needle) {
        let at = from + rel;
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
          from = at + needle.len();
          continue;
        }
        let open = at + needle.len() - 1;
        let Some(close) = match_paren(bytes, open) else { break };
        let count = arg_count(&body[open + 1..close]);
        if count < min {
          out.push(Diagnostic {
            code: "sql567",
            severity: Severity::Error,
            message: format!("`{fname}` needs at least {min} arguments but got {count} (PG error 42883)"),
            range: crate::range_at(start + at, start + close + 1),
          });
        }
        from = close + 1;
      }
    }
  }
}

fn arg_count(inner: &str) -> usize {
  if inner.trim().is_empty() {
    return 0;
  }
  let bytes = inner.as_bytes();
  let mut count = 1usize;
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
      b',' if depth == 0 => count += 1,
      _ => {},
    }
    i += 1;
  }
  count
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
