//! sql528: `REPLACE(s, x, x)` -- the search and replacement strings are
//! identical, so the call returns `s` unchanged. A no-op, almost always a
//! copy-paste slip where the replacement should differ (e.g. `REPLACE(s, '-',
//! '')`). Same idea as NULLIF(x, x) (sql085).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql528"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let needle = "replace(";
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find(needle) {
      let at = from + rel;
      // Whole-word `replace` (excludes regexp_replace, which is a different fn).
      if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
        from = at + needle.len();
        continue;
      }
      let open = at + needle.len() - 1;
      let Some(close) = match_paren(bytes, open) else { break };
      let args = split_args(&body[open + 1..close]);
      if args.len() == 3 && args_identical(args[1].trim(), args[2].trim()) {
        out.push(Diagnostic {
          code: "sql528",
          severity: Severity::Warning,
          message: format!(
            "`REPLACE(..., {}, {})` is a no-op -- search and replacement are identical",
            args[1].trim(),
            args[2].trim()
          ),
          range: crate::range_at(start + at, start + close + 1),
        });
      }
      from = close + 1;
    }
  }
}

/// Case-insensitive unless a string literal is involved (so `REPLACE(s, 'A',
/// 'a')` -- genuinely different -- is not flagged).
fn args_identical(a: &str, b: &str) -> bool {
  if a.contains('\'') || b.contains('\'') {
    a == b
  } else {
    a.eq_ignore_ascii_case(b)
  }
}

fn split_args(inner: &str) -> Vec<&str> {
  let bytes = inner.as_bytes();
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut last = 0usize;
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
      b',' if depth == 0 => {
        out.push(&inner[last..i]);
        last = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  out.push(&inner[last..]);
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
