//! sql652: two common-table expressions in the same `WITH` clause share a name,
//! e.g. `WITH a AS (...), a AS (...)`. PostgreSQL requires CTE names to be
//! unique within a WITH list and raises 42712 ("WITH query name "a" specified
//! more than once"). Rename one of them.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// Balanced `)` for the `(` at `open`.
fn close_of(b: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < b.len() {
    match b[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      }
      _ => {}
    }
    i += 1;
  }
  None
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql652"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let n = ub.len();

    // locate the leading WITH (skip a leading `(` / whitespace)
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("WITH") {
      return;
    }
    let mut i = upper.len() - trimmed.len() + 4;
    // optional RECURSIVE
    let after = upper[i..].trim_start();
    if after.starts_with("RECURSIVE") {
      i = upper.len() - after.len() + 9;
    }

    let mut seen: Vec<String> = Vec::new();
    loop {
      while i < n && ub[i].is_ascii_whitespace() {
        i += 1;
      }
      // read CTE name (optionally double-quoted)
      let name_start = i;
      if i < n && ub[i] == b'"' {
        i += 1;
        while i < n && ub[i] != b'"' {
          i += 1;
        }
        i += 1; // closing quote
      } else {
        while i < n && is_word(ub[i] as char) {
          i += 1;
        }
      }
      if i == name_start {
        return;
      }
      let name = upper[name_start..i].trim_matches('"').to_string();
      // skip whitespace, optional column list `(...)`
      while i < n && ub[i].is_ascii_whitespace() {
        i += 1;
      }
      if i < n && ub[i] == b'(' {
        let Some(c) = close_of(ub, i) else { return };
        i = c + 1;
        while i < n && ub[i].is_ascii_whitespace() {
          i += 1;
        }
      }
      // expect AS
      if i + 2 > n || &ub[i..i + 2] != b"AS" {
        return;
      }
      i += 2;
      while i < n && ub[i].is_ascii_whitespace() {
        i += 1;
      }
      // optional [NOT] MATERIALIZED
      for kw in ["NOT MATERIALIZED", "MATERIALIZED"] {
        if upper[i..].starts_with(kw) {
          i += kw.len();
          while i < n && ub[i].is_ascii_whitespace() {
            i += 1;
          }
          break;
        }
      }
      // CTE body `(...)`
      if i >= n || ub[i] != b'(' {
        return;
      }
      let Some(c) = close_of(ub, i) else { return };
      // duplicate check
      if seen.iter().any(|s| s == &name) {
        out.push(Diagnostic {
          code: "sql652",
          severity: Severity::Error,
          message: format!("WITH query name `{name}` is defined more than once -- PG raises 42712; rename one CTE"),
          range: crate::range_at(start + name_start, start + name_start + name.len()),
        });
      } else {
        seen.push(name);
      }
      i = c + 1;
      // continue only if a comma follows
      while i < n && ub[i].is_ascii_whitespace() {
        i += 1;
      }
      if i < n && ub[i] == b',' {
        i += 1;
        continue;
      }
      return;
    }
  }
}
