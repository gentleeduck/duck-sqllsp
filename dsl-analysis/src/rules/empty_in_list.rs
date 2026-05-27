//! sql234: `WHERE col IN ()` -- literal empty IN list. PG raises
//! 42601 at parse time. Common when generating IN-list from an
//! empty parameter array without guarding.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql234"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = body.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(" IN ") {
      let at = from + rel + " IN ".len();
      let rest = body[at..].trim_start();
      if !rest.starts_with('(') {
        from = at;
        continue;
      }
      let open = at + (body[at..].len() - rest.len());
      let Some(close) = find_matching_paren(body, open) else { break };
      let inner = body[open + 1..close].trim();
      if inner.is_empty() {
        out.push(Diagnostic {
          code: "sql234",
          severity: Severity::Error,
          message: "Empty `IN ()` list -- PG 42601 at parse; guard for empty arrays or use `IN (NULL)` placeholder"
            .into(),
          range: text_size::TextRange::new(((start + open) as u32).into(), ((start + close + 1) as u32).into()),
        });
      }
      from = close + 1;
    }
    let _ = bytes;
  }
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
