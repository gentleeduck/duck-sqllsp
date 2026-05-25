//! sql228: `x = ANY (SELECT 1, 2 FROM ...)` -- the subquery on the
//! RHS of an ANY/ALL/IN must return exactly one column. PG raises
//! 42601 at parse time. Counts top-level commas in the subquery
//! projection.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql228"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    for kw in ["= ANY (", "<> ANY (", "= ALL (", "<> ALL (", " IN ("] {
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(kw) {
        let at = from + rel;
        let open = at + kw.len() - 1;
        let Some(close) = find_matching_paren(body, open) else { from = open; break };
        let inner = &body[open + 1..close];
        let inner_upper = inner.to_ascii_uppercase();
        if !inner_upper.trim_start().starts_with("SELECT") { from = close + 1; continue }
        let proj_end = inner_upper.find(" FROM ").unwrap_or(inner.len());
        let proj = &inner[7..proj_end].trim();
        let cols = 1 + count_top_level_commas(proj);
        if cols > 1 && !proj.contains('*') {
          out.push(Diagnostic {
            code: "sql228",
            severity: Severity::Error,
            message: format!(
              "ANY/ALL/IN subquery returns {cols} columns -- exactly 1 required (PG 42601)"
            ),
            range: text_size::TextRange::new(((start + open) as u32).into(), ((start + close + 1) as u32).into()),
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
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => commas += 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      }
      _ => {}
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
