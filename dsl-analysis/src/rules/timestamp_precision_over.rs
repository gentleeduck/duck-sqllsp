//! sql308: `TIMESTAMP(7)` / `TIME(7)` / `TIMESTAMPTZ(7)` etc. PG
//! caps date/time precision at 6 (microseconds). Higher precisions
//! are silently capped. Hint: drop to (6) or omit.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const TYPES: &[&str] = &["TIMESTAMP", "TIMESTAMPTZ", "TIME", "TIMETZ"];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql308"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    for &ty in TYPES {
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(ty) {
        let at = from + rel;
        if at > 0 {
          let prev = body.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' {
            from = at + ty.len();
            continue;
          }
        }
        let after = at + ty.len();
        if after >= body.len() || body.as_bytes()[after] != b'(' {
          from = after;
          continue;
        }
        let Some(close) = find_matching_paren(body, after) else { break };
        let inside = body[after + 1..close].trim();
        if let Ok(p) = inside.parse::<u32>()
          && p > 6
        {
          out.push(Diagnostic {
            code: "sql308",
            severity: Severity::Hint,
            message: format!(
              "{ty}({p}) -- PG caps date/time precision at 6 (microseconds); higher precision is silently capped"
            ),
            range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
          });
        }
        from = close + 1;
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
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
