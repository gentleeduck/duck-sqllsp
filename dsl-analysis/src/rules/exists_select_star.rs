//! sql227: `EXISTS (SELECT * FROM ...)` -- the projection is
//! discarded; `SELECT 1` is the conventional form and reads more
//! clearly (and avoids the planner expanding * unnecessarily on
//! wide rows in some PG versions).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql227"
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
    while let Some(rel) = upper[from..].find("EXISTS") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 6;
          continue;
        }
      }
      let after = at + "EXISTS".len();
      let post = body[after..].trim_start();
      if !post.starts_with('(') {
        from = after;
        continue;
      }
      let open = after + (body[after..].len() - post.len());
      let Some(close) = find_matching_paren(body, open) else { break };
      let inner = body[open + 1..close].trim();
      let inner_upper = inner.to_ascii_uppercase();
      if inner_upper.starts_with("SELECT *") || inner_upper.starts_with("SELECT  *") {
        out.push(Diagnostic {
          code: "sql227",
          severity: Severity::Hint,
          message: "EXISTS (SELECT * ...) -- projection ignored; `SELECT 1` is the conventional form".into(),
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
