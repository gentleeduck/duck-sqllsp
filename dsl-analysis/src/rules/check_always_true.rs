//! sql244: `CHECK (TRUE)` / `CHECK (1=1)` / `CHECK (1)` constraint
//! is trivially satisfied -- it enforces nothing. Almost always
//! placeholder code that escaped review.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql244"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("CHECK") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 5;
          continue;
        }
      }
      let after = at + "CHECK".len();
      let post = body[after..].trim_start();
      if !post.starts_with('(') {
        from = after;
        continue;
      }
      let open = after + (body[after..].len() - post.len());
      let Some(close) = find_matching_paren(body, open) else { break };
      let inner = body[open + 1..close].trim();
      let inner_upper = inner.to_ascii_uppercase().replace(' ', "");
      let trivial = matches!(inner_upper.as_str(), "TRUE" | "1=1" | "(TRUE)" | "(1=1)" | "1" | "TRUE=TRUE");
      if trivial {
        out.push(Diagnostic {
          code: "sql244",
          severity: Severity::Warning,
          message: format!("CHECK ({}) is trivially true -- constraint enforces nothing", inner),
          range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
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
