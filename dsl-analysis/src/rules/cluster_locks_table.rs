//! sql585: a `CLUSTER` command. It physically rewrites the whole table in
//! index order under an ACCESS EXCLUSIVE lock, blocking every read and write
//! for the entire duration -- on a large table that's a long outage. The
//! ordering also isn't maintained afterwards. For online use, reach for
//! `pg_repack`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql585"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    // Only the CLUSTER *command* (statement starts with it), not
    // `ALTER TABLE ... CLUSTER ON idx` (which just marks the index).
    let lead = upper.len() - upper.trim_start().len();
    let u = upper.trim_start();
    if !(u.starts_with("CLUSTER") && u.as_bytes().get(7).is_none_or(|&b| !is_word(b as char))) {
      return;
    }
    out.push(Diagnostic {
      code: "sql585",
      severity: Severity::Warning,
      message: "CLUSTER rewrites the whole table under an ACCESS EXCLUSIVE lock -- it blocks all access; use pg_repack for online clustering".into(),
      range: crate::range_at(start + lead, start + lead + 7),
    });
  }
}
