//! sql268: `(SELECT ... ORDER BY a) UNION (SELECT ...)` -- ORDER
//! BY inside a UNION branch is allowed only on the LAST branch (and
//! applies to the whole UNION). PG raises 42601 when an earlier
//! branch has ORDER BY without LIMIT/OFFSET.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql268"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(first_union) = upper.find(" UNION ").or_else(|| upper.find(" UNION\n")) else { return };
    // Walk parenthesised SELECT branches before the union.
    let before = &body[..first_union];
    let upper_before = &upper[..first_union];
    let bytes = before.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
      if bytes[i] != b'(' { i += 1; continue }
      let open = i;
      let Some(close) = find_matching_paren(body, open) else { break };
      let inner = &body[open + 1..close];
      let inner_upper = inner.to_ascii_uppercase();
      if inner_upper.contains("SELECT") && inner_upper.contains("ORDER BY")
        && !inner_upper.contains("LIMIT") && !inner_upper.contains("OFFSET") && !inner_upper.contains("FETCH")
      {
        out.push(Diagnostic {
          code: "sql268",
          severity: Severity::Error,
          message: "ORDER BY inside an earlier UNION branch is invalid -- ORDER BY can only appear on the last branch (and applies to the whole UNION); add LIMIT/OFFSET or move ORDER BY outside".into(),
          range: text_size::TextRange::new(((start + open) as u32).into(), ((start + close + 1) as u32).into()),
        });
      }
      i = close + 1;
    }
    let _ = upper_before;
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
