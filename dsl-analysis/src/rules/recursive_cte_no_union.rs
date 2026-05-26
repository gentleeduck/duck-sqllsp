//! sql220: `WITH RECURSIVE t(...) AS (<single SELECT>) ...` -- the
//! recursive CTE body must use UNION [ALL] to combine the anchor +
//! recursive parts. A single SELECT is structurally non-recursive
//! and the RECURSIVE keyword serves no purpose. PG raises at parse
//! when the body actually self-references; this rule catches the
//! more common case where the author wrote RECURSIVE then forgot
//! the recursion.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql220"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = strip_noise(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(rec_at) = upper.find("WITH RECURSIVE") else { return };
    // For each CTE body, extract the parens content and check for UNION inside.
    let after = rec_at + "WITH RECURSIVE".len();
    let bytes = body.as_bytes();
    let mut i = after;
    while i < bytes.len() {
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      // CTE name + optional col list + AS.
      while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1 }
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      if i < bytes.len() && bytes[i] == b'(' {
        let Some(close) = find_matching_paren(body, i) else { break };
        i = close + 1;
      }
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      // AS
      if i + 2 > bytes.len() || !upper[i..].starts_with("AS") { break }
      i += 2;
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      // optional MATERIALIZED / NOT MATERIALIZED
      if upper[i..].starts_with("MATERIALIZED") { i += "MATERIALIZED".len(); }
      else if upper[i..].starts_with("NOT MATERIALIZED") { i += "NOT MATERIALIZED".len(); }
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      if i >= bytes.len() || bytes[i] != b'(' { break }
      let body_open = i;
      let Some(body_close) = find_matching_paren(body, body_open) else { break };
      let cte_body = &body[body_open + 1..body_close];
      let cte_upper = cte_body.to_ascii_uppercase();
      if !cte_upper.contains(" UNION ") && !cte_upper.contains("\nUNION ") {
        out.push(Diagnostic {
          code: "sql220",
          severity: Severity::Warning,
          message: "WITH RECURSIVE CTE body has no UNION [ALL] -- non-recursive form, drop RECURSIVE or add the recursive UNION".into(),
          range: text_size::TextRange::new(((start + body_open) as u32).into(), ((start + body_close + 1) as u32).into()),
        });
      }
      i = body_close + 1;
      while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1 }
      if i < bytes.len() && bytes[i] == b',' { i += 1; continue }
      break;
    }
  }
}

fn strip_noise(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' { out[i] = b' '; i += 1 }
      continue;
    }
    if i + 1 < n && out[i] == b'/' && out[i + 1] == b'*' {
      let mut depth = 1u32;
      out[i] = b' '; out[i + 1] = b' '; i += 2;
      while i + 1 < n && depth > 0 {
        if out[i] == b'/' && out[i + 1] == b'*' { depth += 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else if out[i] == b'*' && out[i + 1] == b'/' { depth -= 1; out[i] = b' '; out[i + 1] = b' '; i += 2; }
        else { out[i] = b' '; i += 1; }
      }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' '; i += 1;
      while i < n && out[i] != b'\'' { out[i] = b' '; i += 1 }
      if i < n { out[i] = b' '; i += 1 }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
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
