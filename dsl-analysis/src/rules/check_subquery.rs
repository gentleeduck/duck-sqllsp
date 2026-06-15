//! sql606: a `CHECK` constraint whose expression contains a subquery (e.g.
//! `CHECK (col IN (SELECT ...))`). PostgreSQL forbids subqueries in CHECK
//! expressions and rejects the statement (0A000). Enforce cross-row rules with
//! a trigger or a foreign key instead.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

fn contains_word(haystack: &str, needle: &str) -> bool {
  let hb = haystack.as_bytes();
  let nl = needle.len();
  let mut i = 0usize;
  while i + nl <= hb.len() {
    if &hb[i..i + nl] == needle.as_bytes()
      && (i == 0 || !is_word(hb[i - 1] as char))
      && (i + nl == hb.len() || !is_word(hb[i + nl] as char))
    {
      return true;
    }
    i += 1;
  }
  false
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql606"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 5 <= n {
      if &ub[i..i + 5] == b"CHECK"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 5 == n || !is_word(ub[i + 5] as char))
      {
        // skip whitespace to the opening paren
        let mut j = i + 5;
        while j < n && ub[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && ub[j] == b'(' {
          let mut depth = 0i32;
          let mut k = j;
          let mut close = None;
          while k < n {
            match ub[k] {
              b'(' => depth += 1,
              b')' => {
                depth -= 1;
                if depth == 0 {
                  close = Some(k);
                  break;
                }
              }
              _ => {}
            }
            k += 1;
          }
          if let Some(close) = close {
            if contains_word(&upper[j + 1..close], "SELECT") {
              out.push(Diagnostic {
                code: "sql606",
                severity: Severity::Error,
                message: "subquery in a CHECK constraint -- PostgreSQL forbids this; use a trigger or foreign key instead".into(),
                range: crate::range_at(start + i, start + i + 5),
              });
            }
            i = close + 1;
            continue;
          }
        }
      }
      i += 1;
    }
  }
}
