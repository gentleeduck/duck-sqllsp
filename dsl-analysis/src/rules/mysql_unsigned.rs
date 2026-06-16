//! sql599: `int unsigned` / `bigint unsigned` -- MySQL's UNSIGNED integer
//! modifier. PostgreSQL has no unsigned integer types and rejects the keyword.
//! To enforce non-negativity, keep the signed type and add `CHECK (col >= 0)`,
//! or step up to a wider type (`bigint` for an unsigned `int`) if you need the
//! extra range.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql599"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 8 <= n {
      if &ub[i..i + 8] == b"UNSIGNED"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 8 == n || !is_word(ub[i + 8] as char))
      {
        out.push(Diagnostic {
          code: "sql599",
          severity: Severity::Error,
          message: "`UNSIGNED` is a MySQL type modifier -- PostgreSQL has no unsigned integers; use a CHECK (col >= 0) or a wider type".into(),
          range: crate::range_at(start + i, start + i + 8),
        });
        return;
      }
      i += 1;
    }
  }
}
