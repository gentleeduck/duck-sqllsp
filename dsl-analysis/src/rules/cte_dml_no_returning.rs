//! sql229: `WITH foo AS (UPDATE/INSERT/DELETE ...) SELECT * FROM foo`
//! where the data-modifying CTE has no RETURNING clause. PG raises
//! 0A000 "WITH clause containing a data-modifying statement must
//! have a RETURNING clause" when the outer query references the CTE.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql229"
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
    let Some(with_at) = upper.find("WITH ") else { return };
    let after = with_at + "WITH ".len();
    let bytes = body.as_bytes();
    let mut i = after;
    let mut cte_names: Vec<(String, usize, usize, bool)> = Vec::new(); // (name, open, close, has_returning)
    loop {
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      if i >= bytes.len() { break }
      if upper[i..].starts_with("RECURSIVE ") { i += "RECURSIVE ".len(); continue }
      let name_start = i;
      while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1 }
      if i == name_start { break }
      let name = body[name_start..i].to_string();
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      // optional (col, col)
      if i < bytes.len() && bytes[i] == b'(' {
        let Some(close) = find_matching_paren(body, i) else { break };
        i = close + 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      }
      // AS
      if i + 2 > bytes.len() || !upper[i..].starts_with("AS") { break }
      i += 2;
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      if upper[i..].starts_with("MATERIALIZED ") { i += "MATERIALIZED ".len(); }
      else if upper[i..].starts_with("NOT MATERIALIZED ") { i += "NOT MATERIALIZED ".len(); }
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      if i >= bytes.len() || bytes[i] != b'(' { break }
      let body_open = i;
      let Some(body_close) = find_matching_paren(body, body_open) else { break };
      let cte_body = &body[body_open + 1..body_close];
      let cb_upper = cte_body.to_ascii_uppercase();
      let is_dml = cb_upper.trim_start().starts_with("INSERT")
        || cb_upper.trim_start().starts_with("UPDATE")
        || cb_upper.trim_start().starts_with("DELETE");
      if is_dml {
        let has_returning = cb_upper.contains("RETURNING");
        cte_names.push((name, body_open, body_close, has_returning));
      }
      i = body_close + 1;
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      if i < bytes.len() && bytes[i] == b',' { i += 1; continue }
      break;
    }
    // The outer query is what follows.
    let outer_upper = body[i..].to_ascii_uppercase();
    for (name, open, close, has_returning) in cte_names {
      if has_returning { continue }
      let ref_pattern = name.to_ascii_uppercase();
      if outer_upper.contains(&ref_pattern) {
        out.push(Diagnostic {
          code: "sql229",
          severity: Severity::Error,
          message: format!(
            "Data-modifying CTE `{name}` referenced by outer query without RETURNING -- PG raises 0A000; add RETURNING"
          ),
          range: text_size::TextRange::new(((start + open) as u32).into(), ((start + close + 1) as u32).into()),
        });
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
