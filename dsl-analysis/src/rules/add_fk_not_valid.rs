//! sql589: `ALTER TABLE t ADD CONSTRAINT fk FOREIGN KEY (a) REFERENCES b (c)`
//! without `NOT VALID`. Adding a validated foreign key scans every existing
//! row to check it, holding a lock that blocks writes on both tables for the
//! duration. Add it `NOT VALID` (cheap, only new rows are checked), then
//! `VALIDATE CONSTRAINT` in a separate step (takes only a SHARE UPDATE
//! EXCLUSIVE lock). (Companion to sql280 for CHECK constraints.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql589"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("ALTER TABLE") || !upper.contains("ADD") || upper.contains("NOT VALID") {
      return;
    }
    let ub = upper.as_bytes();
    if let Some(at) = find_phrase(ub, b"FOREIGN KEY") {
      out.push(Diagnostic {
        code: "sql589",
        severity: Severity::Warning,
        message: "adding a FOREIGN KEY without NOT VALID scans the whole table under a write-blocking lock -- add it NOT VALID, then VALIDATE CONSTRAINT separately".into(),
        range: crate::range_at(start + at, start + at + 11),
      });
    }
  }
}

fn find_phrase(ub: &[u8], kw: &[u8]) -> Option<usize> {
  let n = ub.len();
  let m = kw.len();
  let mut i = 0usize;
  while i + m <= n {
    if ub[i..i + m] == *kw
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + m == n || !is_word(ub[i + m] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}
