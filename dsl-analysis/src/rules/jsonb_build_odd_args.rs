//! sql266: `jsonb_build_object(k1, v1, k2)` -- argument count must
//! be even (alternating key/value). PG raises 22023 at runtime.
//! Same for `json_build_object`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const FNS: &[&str] = &["jsonb_build_object(", "json_build_object("];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql266"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    for &fname in FNS {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(fname) {
        let at = from + rel;
        if at > 0 {
          let prev = body.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            from = at + fname.len();
            continue;
          }
        }
        let open = at + fname.len() - 1;
        let Some(close) = find_matching_paren(body, open) else { break };
        let inner = &body[open + 1..close];
        if inner.trim().is_empty() {
          from = close + 1;
          continue;
        }
        let args = 1 + count_top_level_commas(inner);
        if !args.is_multiple_of(2) {
          out.push(Diagnostic {
            code: "sql266",
            severity: Severity::Error,
            message: format!(
              "`{}` has {args} args -- must be even (key/value pairs); PG raises 22023",
              fname.trim_end_matches('('),
            ),
            range: crate::range_at(start + at, start + close + 1),
          });
        }
        from = close + 1;
      }
    }
  }
}

fn count_top_level_commas(text: &str) -> usize {
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut commas = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b',' if depth == 0 => commas += 1,
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
  commas
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
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
