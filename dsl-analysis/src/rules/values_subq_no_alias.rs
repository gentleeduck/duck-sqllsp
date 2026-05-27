//! sql243: `FROM (VALUES (1, 2)) WHERE ...` -- a VALUES-derived
//! relation needs an alias plus a column list. PG raises 42601
//! "subquery in FROM must have an alias" without it.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql243"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("(VALUES") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 7;
          continue;
        }
      }
      let Some(close) = find_matching_paren(body, at) else { break };
      let post = body[close + 1..].trim_start();
      let post_upper = post.to_ascii_uppercase();
      let has_as = post_upper.starts_with("AS ");
      // Lenient: accept any identifier-with-paren-list (e.g. `t(a,b)`) right after.
      let has_implicit = !has_as && post.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_');
      if has_as || has_implicit {
        from = close + 1;
        continue;
      }
      out.push(Diagnostic {
        code: "sql243",
        severity: Severity::Error,
        message: "(VALUES ...) in FROM needs an alias -- add `AS t(col1, col2)` (PG 42601)".into(),
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
