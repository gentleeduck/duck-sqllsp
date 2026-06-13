//! sql534: `GREATEST(x, x)` / `LEAST(a, b, a)` -- a duplicate argument. The
//! max / min is unaffected by repeating a value, so the extra argument is
//! dead. `GREATEST(x, x)` in particular just returns `x`. Usually a typo for
//! a different second argument. (Mirrors sql417 for COALESCE.)

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql534"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    for fname in ["greatest(", "least("] {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(fname) {
        let at = from + rel;
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
          from = at + fname.len();
          continue;
        }
        let open = at + fname.len() - 1;
        let Some(close) = match_paren(bytes, open) else { break };
        let args: Vec<&str> = split_args(&body[open + 1..close]).iter().map(|a| a.trim()).collect();
        if has_duplicate(&args) {
          out.push(Diagnostic {
            code: "sql534",
            severity: Severity::Warning,
            message: format!(
              "duplicate argument in `{}(...)` -- repeating a value has no effect on the {}",
              fname.trim_end_matches('('),
              if fname.starts_with('g') { "maximum" } else { "minimum" }
            ),
            range: crate::range_at(start + at, start + close + 1),
          });
        }
        from = close + 1;
      }
    }
  }
}

fn has_duplicate(args: &[&str]) -> bool {
  for i in 0..args.len() {
    for j in (i + 1)..args.len() {
      if !args[i].is_empty() && idents_eq(args[i], args[j]) {
        return true;
      }
    }
  }
  false
}

/// Case-insensitive unless a string literal is involved.
fn idents_eq(a: &str, b: &str) -> bool {
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
