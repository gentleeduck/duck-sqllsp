//! sql583: `EXISTS (SELECT ... GROUP BY x)` with no HAVING -- the GROUP BY is
//! dead weight. EXISTS only checks for at least one row; grouping the rows
//! first can't change whether any exist (a non-empty input always yields at
//! least one group). With a HAVING it *can* matter, so that case is left
//! alone. (Companion to sql525 / sql569 / sql570 for LIMIT / ORDER BY /
//! DISTINCT in EXISTS.)

use crate::clause_scan::{find_clause, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql583"
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
      if ub[i..i + 6] != *b"EXISTS" || (i > 0 && is_word(ub[i - 1] as char)) || (i + 6 < n && is_word(ub[i + 6] as char)) {
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
      if let Some(rel) = find_clause(inner, b"GROUP")
        && find_clause(inner, b"HAVING").is_none()
      {
        let at = p + 1 + rel;
        out.push(Diagnostic {
          code: "sql583",
          severity: Severity::Hint,
          message: "GROUP BY inside EXISTS (without HAVING) is pointless -- EXISTS only checks for any row".into(),
          range: crate::range_at(start + at, start + at + 5),
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
