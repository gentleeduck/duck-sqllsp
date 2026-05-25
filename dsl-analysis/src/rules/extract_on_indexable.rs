//! sql269: `WHERE EXTRACT(YEAR FROM ts) = 2024` -- wrapping a
//! timestamp column in EXTRACT prevents the planner from using a
//! btree index. Suggest a range predicate
//! (`ts >= '2024-01-01' AND ts < '2025-01-01'`) so the index applies.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql269"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("WHERE") { return }
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("EXTRACT(") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' { from = at + 8; continue }
      }
      let open = at + "EXTRACT(".len() - 1;
      let Some(close) = find_matching_paren(body, open) else { from = open; break };
      let post = body[close + 1..].trim_start();
      if !post.starts_with('=') { from = close + 1; continue }
      out.push(Diagnostic {
        code: "sql269",
        severity: Severity::Hint,
        message: "EXTRACT(... FROM col) = N blocks btree index on col -- prefer a range predicate (e.g. col >= 'YYYY-01-01' AND col < 'YYYY+1-01-01')".into(),
        range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
      });
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
