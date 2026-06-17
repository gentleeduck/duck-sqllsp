//! sql580: `CREATE UNLOGGED TABLE ...` (or `ALTER TABLE ... SET UNLOGGED`) --
//! unlogged tables skip the WAL, so they're fast to write but their entire
//! contents are TRUNCATED on a crash or unclean restart, and they aren't
//! replicated to standbys. Fine for scratch/cache data, but a data-loss
//! surprise if the table holds anything you expect to keep.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql580"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("TABLE") {
      return;
    }
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 8 <= n {
      if &ub[i..i + 8] == b"UNLOGGED"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 8 == n || !is_word(ub[i + 8] as char))
      {
        out.push(Diagnostic {
          code: "sql580",
          severity: Severity::Hint,
          message: "UNLOGGED table -- its contents are wiped on a crash and not replicated; only use it for disposable data".into(),
          range: crate::range_at(start + i, start + i + 8),
        });
        return;
      }
      i += 1;
    }
  }
}
