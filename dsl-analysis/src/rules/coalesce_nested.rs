//! sql530: `COALESCE(COALESCE(a, b), c)` -- a COALESCE whose argument is
//! itself a COALESCE. The two collapse into one `COALESCE(a, b, c)`, which is
//! shorter and lets the planner stop at the first non-NULL without an extra
//! nesting level. Usually an artifact of incremental edits.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql530"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let needle = "coalesce(";
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find(needle) {
      let at = from + rel;
      if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
        from = at + needle.len();
        continue;
      }
      let open = at + needle.len() - 1;
      let Some(close) = match_paren(bytes, open) else { break };
      let inner = &body[open + 1..close];
      // Flag when any top-level argument is itself a whole COALESCE call.
      if split_args(inner).iter().any(|a| is_coalesce_call(a)) {
        out.push(Diagnostic {
          code: "sql530",
          severity: Severity::Hint,
          message: "nested COALESCE -- flatten into a single COALESCE(...) call".into(),
          range: crate::range_at(start + at, start + close + 1),
        });
      }
      from = open + 1;
    }
  }
}

/// True when `arg` (trimmed) is exactly a `COALESCE(...)` call -- i.e. the
/// keyword opens at the start and its matching paren closes at the end.
fn is_coalesce_call(arg: &str) -> bool {
  let t = arg.trim();
  let tl = t.to_ascii_lowercase();
  if !tl.starts_with("coalesce") {
    return false;
  }
  let rest = t["coalesce".len()..].trim_start();
  if !rest.starts_with('(') {
    return false;
  }
  let open = t.len() - rest.len();
  match_paren(t.as_bytes(), open) == Some(t.len() - 1)
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
