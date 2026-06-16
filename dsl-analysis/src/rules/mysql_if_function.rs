//! sql621: the MySQL `IF(cond, then, else)` function. PostgreSQL has no scalar
//! `IF()` function (42883); use a `CASE WHEN cond THEN ... ELSE ... END`
//! expression (or `COALESCE` / `NULLIF` for the simple shapes).
//!
//! Carefully distinguished from PL/pgSQL's `IF ... THEN` control statement: the
//! function form has a comma-separated argument list and is *not* followed by
//! `THEN`.

use crate::clause_scan::{is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql621"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let n = lb.len();
    let mut i = 0usize;
    while i + 2 <= n {
      if &lb[i..i + 2] == b"if"
        && (i == 0 || !is_word(lb[i - 1] as char))
      {
        // require `(` after optional whitespace
        let mut j = i + 2;
        while j < n && lb[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && lb[j] == b'(' {
          // balance the parens
          let mut depth = 0i32;
          let mut k = j;
          let mut close = None;
          while k < n {
            match lb[k] {
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
            // PL/pgSQL `IF (...) THEN` -> control statement, skip
            let mut t = close + 1;
            while t < n && lb[t].is_ascii_whitespace() {
              t += 1;
            }
            let followed_by_then = t + 4 <= n && &lb[t..t + 4] == b"then";
            let multi_arg = split_top_level(&body[j + 1..close]).len() >= 2;
            if multi_arg && !followed_by_then {
              out.push(Diagnostic {
                code: "sql621",
                severity: Severity::Error,
                message: "`IF(...)` is a MySQL function -- PostgreSQL has no scalar IF(); use a CASE expression".into(),
                range: crate::range_at(start + i, start + i + 2),
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
