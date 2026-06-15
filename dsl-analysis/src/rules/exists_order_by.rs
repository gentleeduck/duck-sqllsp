//! sql569: `EXISTS (SELECT ... ORDER BY ...)` -- ordering inside an EXISTS
//! subquery is dead weight. EXISTS only asks "is there at least one row?",
//! which is independent of order, so the planner discards the sort anyway.
//! Drop the ORDER BY. (Companion to sql525, which handles LIMIT in EXISTS.)

use crate::clause_scan::{find_clause, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql569"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 6 <= n {
      if ub[i..i + 6] != *b"EXISTS"
        || (i > 0 && is_word(ub[i - 1] as char))
        || (i + 6 < n && is_word(ub[i + 6] as char))
      {
        i += 1;
        continue;
      }
      let mut p = i + 6;
      while p < n && ub[p].is_ascii_whitespace() {
        p += 1;
      }
      if ub.get(p) != Some(&b'(') {
        i += 6;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let inner = &ub[p + 1..close];
      // ORDER BY at the subquery's own depth (not inside a window OVER()).
      if let Some(rel) = find_clause(inner, b"ORDER") {
        let at = p + 1 + rel;
        out.push(Diagnostic {
          code: "sql569",
          severity: Severity::Hint,
          message: "ORDER BY inside EXISTS is pointless -- EXISTS ignores ordering".into(),
          range: crate::range_at(start + at, start + (at + 5)),
        });
      }
      i = close + 1;
    }
  }
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
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
