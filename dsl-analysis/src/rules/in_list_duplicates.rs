//! sql306: `WHERE id IN (1, 1, 2)` -- duplicate literal in IN list.
//! Planner dedups but the query is larger + harder to read.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql306"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(" IN (") {
      let at = from + rel + " IN ".len();
      let open = at;
      let Some(close) = find_matching_paren(body, open) else { break };
      let inner = &body[open + 1..close];
      let inner_upper = inner.to_ascii_uppercase();
      if inner_upper.trim_start().starts_with("SELECT") {
        from = close + 1;
        continue;
      }
      let items: Vec<String> = inner.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
      if items.len() < 3 {
        from = close + 1;
        continue;
      }
      let mut sorted = items.clone();
      sorted.sort();
      let mut dedup = sorted.clone();
      dedup.dedup();
      if dedup.len() != sorted.len() {
        out.push(Diagnostic {
          code: "sql306",
          severity: Severity::Hint,
          message: "Duplicate literals in IN list -- planner dedups but query is unnecessarily larger".into(),
          range: text_size::TextRange::new(((start + open) as u32).into(), ((start + close + 1) as u32).into()),
        });
      }
      from = close + 1;
    }
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
