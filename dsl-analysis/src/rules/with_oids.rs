//! sql615: the `WITH OIDS` table option. System OID columns on user tables were
//! removed in PostgreSQL 12, and `WITH OIDS` (in CREATE TABLE or
//! `ALTER TABLE ... SET WITH OIDS`) now raises 42601. Remove the clause; if you
//! relied on a hidden row identifier, add an explicit identity/serial column.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql615"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let needle = b"WITH OIDS";
    let len = needle.len();
    let mut i = 0usize;
    while i + len <= n {
      if &ub[i..i + len] == needle
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub.get(i + len).is_none_or(|&b| !is_word(b as char))
      {
        out.push(Diagnostic {
          code: "sql615",
          severity: Severity::Error,
          message: "`WITH OIDS` was removed in PostgreSQL 12 -- remove the clause; use an explicit identity/serial column for a row identifier".into(),
          range: crate::range_at(start + i, start + i + len),
        });
        return;
      }
      i += 1;
    }
  }
}
