//! sql603: `... MINUS ...` -- Oracle's set-difference operator. PostgreSQL
//! spells it `EXCEPT` (and `EXCEPT ALL` to keep duplicates). Word-bounded so an
//! identifier like `minus_balance` isn't matched.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql603"
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
      if &ub[i..i + 5] == b"MINUS"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 5 == n || !is_word(ub[i + 5] as char))
      {
        out.push(Diagnostic {
          code: "sql603",
          severity: Severity::Error,
          message: "`MINUS` is the Oracle set operator -- PostgreSQL uses `EXCEPT`".into(),
          range: crate::range_at(start + i, start + i + 5),
        });
        return;
      }
      i += 1;
    }
  }
}
