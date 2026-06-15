//! sql613: `col ... GENERATED ALWAYS AS (expr)` without the `STORED` keyword
//! (or written `... VIRTUAL`). PostgreSQL only supports STORED generated
//! columns; a missing or VIRTUAL specification is a syntax error
//! ("only STORED generated columns are supported"). MySQL and SQL Server default
//! to virtual columns, so this is a frequent port mistake -- append `STORED`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

fn matching_paren(s: &str, open: usize) -> Option<usize> {
  let b = s.as_bytes();
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
    "sql613"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("GENERATED ALWAYS AS") {
      let at = from + rel;
      let after = at + "GENERATED ALWAYS AS".len();
      let rest = body[after..].trim_start();
      // an expression-generated column is `AS (expr)`; `AS IDENTITY` is a
      // different (supported) feature, so only act when a paren follows.
      if !rest.starts_with('(') {
        from = after;
        continue;
      }
      let abs_open = after + (body[after..].len() - rest.len());
      let Some(close) = matching_paren(body, abs_open) else {
        from = after;
        continue;
      };
      let trailing = body[close + 1..].trim_start().to_ascii_uppercase();
      if !trailing.starts_with("STORED") {
        out.push(Diagnostic {
          code: "sql613",
          severity: Severity::Error,
          message: "generated column is missing STORED -- PostgreSQL only supports STORED generated columns (VIRTUAL is unsupported); append STORED".into(),
          range: crate::range_at(start + at, start + close + 1),
        });
      }
      from = close + 1;
    }
  }
}
