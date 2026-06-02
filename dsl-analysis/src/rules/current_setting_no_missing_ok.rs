//! sql256: `current_setting('foo')` -- if the GUC isn't set,
//! PG raises 42704. The 2-arg form `current_setting('foo', true)`
//! returns NULL instead, which is almost always what callers want
//! when reading optional settings. Hint: pass `missing_ok=true`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql256"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("current_setting(") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 16;
          continue;
        }
      }
      let open = at + "current_setting(".len() - 1;
      let Some(close) = find_matching_paren(body, open) else { break };
      let args = body[open + 1..close].trim();
      if !args.contains(',') {
        out.push(Diagnostic {
          code: "sql256",
          severity: Severity::Hint,
          message: format!(
            "`current_setting({})` -- one-arg form raises 42704 on unset GUC; pass `missing_ok=true` (2-arg form returns NULL)",
            args,
          ),
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
