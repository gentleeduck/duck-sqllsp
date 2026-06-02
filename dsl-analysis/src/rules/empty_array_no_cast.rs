//! sql303: `ARRAY[]` (empty constructor) without a `::type[]` cast.
//! PG raises 42P18 "cannot determine type of empty array".
//! Suggest e.g. `ARRAY[]::int[]`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql303"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("ARRAY[") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 6;
          continue;
        }
      }
      let open = at + "ARRAY[".len() - 1;
      let Some(close) = find_matching_bracket(body, open) else { break };
      let inner = body[open + 1..close].trim();
      if !inner.is_empty() {
        from = close + 1;
        continue;
      }
      let after = body[close + 1..].trim_start();
      if after.starts_with("::") {
        from = close + 1;
        continue;
      }
      out.push(Diagnostic {
        code: "sql303",
        severity: Severity::Error,
        message:
          "Empty ARRAY[] without `::type[]` cast -- PG 42P18 cannot determine type; write `ARRAY[]::int[]` or similar"
            .into(),
        range: crate::range_at(start + at, start + close + 1),
      });
      from = close + 1;
    }
  }
}

fn find_matching_bracket(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'[' => depth += 1,
      b']' => {
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
