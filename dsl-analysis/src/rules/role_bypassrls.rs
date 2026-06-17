//! sql573: `CREATE ROLE etl BYPASSRLS` / `ALTER ROLE app BYPASSRLS` -- the
//! BYPASSRLS attribute lets the role skip every row-level-security policy on
//! every table. That quietly defeats RLS for that role; grant it only to
//! trusted admin/maintenance roles, and prefer per-table policies otherwise.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql573"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !(upper.contains("ROLE") || upper.contains("USER")) {
      return;
    }
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 9 <= n {
      // Word-bounded BYPASSRLS; leading boundary excludes NOBYPASSRLS.
      if &ub[i..i + 9] == b"BYPASSRLS"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 9 == n || !is_word(ub[i + 9] as char))
      {
        out.push(Diagnostic {
          code: "sql573",
          severity: Severity::Warning,
          message: "BYPASSRLS lets this role skip all row-level security policies -- grant it only to trusted roles".into(),
          range: crate::range_at(start + i, start + i + 9),
        });
        return;
      }
      i += 1;
    }
  }
}
