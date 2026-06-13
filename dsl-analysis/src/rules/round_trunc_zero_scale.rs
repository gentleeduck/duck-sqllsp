//! sql538: `ROUND(x, 0)` / `TRUNC(x, 0)` -- the explicit scale of 0 is
//! redundant; the single-argument `ROUND(x)` / `TRUNC(x)` already rounds /
//! truncates to zero decimal places. Harmless but noise, and a `, 0` often
//! signals a half-finished edit (the author meant a real scale).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql538"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    for fname in ["round(", "trunc("] {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(fname) {
        let at = from + rel;
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
          from = at + fname.len();
          continue;
        }
        let open = at + fname.len() - 1;
        let Some(close) = match_paren(bytes, open) else { break };
        let args = split_args(&body[open + 1..close]);
        if args.len() == 2 && args[1].trim() == "0" {
          let name = fname.trim_end_matches('(');
          out.push(Diagnostic {
            code: "sql538",
            severity: Severity::Hint,
            message: format!("redundant scale -- `{name}(x, 0)` is the same as `{name}(x)`"),
            range: crate::range_at(start + at, start + close + 1),
          });
        }
        from = close + 1;
      }
    }
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
