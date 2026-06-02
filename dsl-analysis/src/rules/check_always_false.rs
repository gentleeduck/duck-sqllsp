//! sql273: `CHECK (FALSE)` / `CHECK (0)` constraint rejects every
//! row. Almost certainly a placeholder that escaped review.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql273"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
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
      let canon = inner.to_ascii_uppercase().replace(' ', "");
      let always_false = matches!(
        canon.as_str(),
        "FALSE" | "0" | "1=0" | "0=1" | "(FALSE)" | "(0)" | "(1=0)" | "FALSE=TRUE" | "TRUE=FALSE"
      );
      if always_false {
        out.push(Diagnostic {
          code: "sql273",
          severity: Severity::Error,
          message: format!("CHECK ({inner}) is trivially false -- constraint rejects EVERY row"),
          range: crate::range_at(start + at, start + close + 1),
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
