//! sql525: `EXISTS (SELECT ... LIMIT 1)` -- the LIMIT is dead weight. EXISTS
//! short-circuits as soon as the subquery yields a single row, so capping it
//! changes nothing about the result and only adds noise (and a needless node
//! the planner has to reason about). Drop the LIMIT.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const STOPWORDS: &[&str] = &["OFFSET", "FOR", "FETCH"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql525"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
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
      // LIMIT at the EXISTS subquery's own depth (not inside a nested
      // derived table, where a LIMIT can actually matter).
      let inner = &ub[p + 1..close];
      if let Some(lrel) = find_clause(inner, b"LIMIT") {
        let limit_at = p + 1 + lrel;
        let limit_end = find_clause_end(ub, limit_at + 5, STOPWORDS).min(close);
        out.push(Diagnostic {
          code: "sql525",
          severity: Severity::Hint,
          message: "LIMIT inside EXISTS is redundant -- EXISTS already stops at the first row".into(),
          range: crate::range_at(start + limit_at, start + body[..limit_end].trim_end().len()),
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
