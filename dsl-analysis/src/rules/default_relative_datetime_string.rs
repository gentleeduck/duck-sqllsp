//! sql641: a `DEFAULT` of the special relative date/time strings `'now'`,
//! `'today'`, `'tomorrow'`, or `'yesterday'`. PostgreSQL resolves these special
//! values *when the default expression is created* (at DDL time), not at each
//! INSERT -- so `created_at timestamptz DEFAULT 'now'` freezes every row to the
//! moment the table was defined. Use the functions `now()` /
//! `CURRENT_TIMESTAMP` / `CURRENT_DATE`, which are evaluated per row.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const SPECIAL: &[&str] = &["now", "today", "tomorrow", "yesterday"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql641"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 7 <= n {
      if &ub[i..i + 7] == b"DEFAULT"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub.get(i + 7).is_none_or(|&b| !is_word(b as char))
      {
        let mut j = i + 7;
        while j < n && ub[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && body.as_bytes()[j] == b'\'' {
          // read the string literal
          let mut k = j + 1;
          while k < n && body.as_bytes()[k] != b'\'' {
            k += 1;
          }
          if k < n {
            let lit = body[j + 1..k].trim().to_ascii_lowercase();
            if SPECIAL.contains(&lit.as_str()) {
              out.push(Diagnostic {
                code: "sql641",
                severity: Severity::Warning,
                message: format!("DEFAULT '{lit}' is resolved once at DDL time and freezes every row -- use a function like `now()` / `CURRENT_DATE` instead"),
                range: crate::range_at(start + j, start + k + 1),
              });
            }
          }
        }
      }
      i += 1;
    }
  }
}
