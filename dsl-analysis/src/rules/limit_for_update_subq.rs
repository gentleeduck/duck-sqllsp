//! sql222: `SELECT * FROM (SELECT ... LIMIT N) FOR UPDATE` -- the
//! outer FOR UPDATE locks every row matched by the inner SELECT,
//! not just the first N. The intended form is `SELECT ... FOR UPDATE
//! LIMIT N` directly inside the inner query. PG silently does the
//! wrong thing here so the lint is the only signal.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql222"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(_) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("FOR UPDATE") && !upper.contains("FOR SHARE") { return }
    // Look for subqueries that contain LIMIT.
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
      if bytes[i] == b'(' {
        let open = i;
        let Some(close) = find_matching_paren(body, open) else { break };
        let inner = &body[open + 1..close];
        let inner_upper = inner.to_ascii_uppercase();
        if inner_upper.contains("LIMIT") && (inner_upper.contains("SELECT")) {
          let after = body[close + 1..].to_ascii_uppercase();
          if after.trim_start().starts_with("FOR UPDATE") || after.trim_start().starts_with("FOR SHARE") {
            out.push(Diagnostic {
              code: "sql222",
              severity: Severity::Warning,
              message: "Outer FOR UPDATE locks every row of inner subquery (not just LIMIT N) -- move FOR UPDATE inside subquery for correct row-locking".into(),
              range: text_size::TextRange::new(((start + open) as u32).into(), ((start + close + 1) as u32).into()),
            });
          }
        }
        i = close + 1;
      } else {
        i += 1;
      }
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
      b')' => { depth -= 1; if depth == 0 { return Some(i); } }
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      }
      _ => {}
    }
    i += 1;
  }
  None
}
