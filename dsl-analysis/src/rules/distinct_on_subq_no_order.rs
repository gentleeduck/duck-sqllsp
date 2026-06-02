//! sql263: `SELECT * FROM (SELECT DISTINCT ON (k) ... FROM t) sub`
//! without an ORDER BY inside the subquery. DISTINCT ON picks the
//! "first" row per group based on the inner ORDER BY -- without it
//! PG picks an arbitrary row and the result is non-deterministic.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql263"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("DISTINCT ON") {
      let at = from + rel;
      // Walk back to enclosing `(`.
      let pre = &body[..at];
      let Some(open) = pre.rfind('(') else {
        from = at + 11;
        continue;
      };
      let Some(close) = find_matching_paren(body, open) else { break };
      let inner = &body[open + 1..close];
      let inner_upper = inner.to_ascii_uppercase();
      if !inner_upper.contains("ORDER BY") {
        out.push(Diagnostic {
          code: "sql263",
          severity: Severity::Warning,
          message: "DISTINCT ON without inner ORDER BY -- which row PG keeps is non-deterministic; add ORDER BY matching the DISTINCT ON keys".into(),
          range: crate::range_at(start + open, start + close + 1),
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
