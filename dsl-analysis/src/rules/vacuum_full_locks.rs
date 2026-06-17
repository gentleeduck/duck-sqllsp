//! sql586: `VACUUM FULL` rewrites the entire table (and its indexes) into new
//! files under an ACCESS EXCLUSIVE lock, blocking all reads and writes until
//! it finishes -- and it needs free disk space roughly equal to the table.
//! Plain `VACUUM` reclaims space online; `pg_repack` compacts without the long
//! lock. Reserve `VACUUM FULL` for a planned maintenance window.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql586"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let u = upper.trim_start();
    let lead = upper.len() - u.len();
    if !(u.starts_with("VACUUM") && u.as_bytes().get(6).is_none_or(|&b| !is_word(b as char))) {
      return;
    }
    // `FULL` as a word anywhere in the VACUUM (covers `VACUUM FULL t` and
    // `VACUUM (FULL) t`).
    let ub = upper.as_bytes();
    if let Some(at) = find_word(ub, b"FULL", lead + 6) {
      out.push(Diagnostic {
        code: "sql586",
        severity: Severity::Warning,
        message: "VACUUM FULL rewrites the whole table under an ACCESS EXCLUSIVE lock and needs ~table-sized free disk -- use plain VACUUM or pg_repack online".into(),
        range: crate::range_at(start + lead, start + at + 4),
      });
    }
  }
}

fn find_word(ub: &[u8], kw: &[u8], from: usize) -> Option<usize> {
  let n = ub.len();
  let m = kw.len();
  let mut i = from;
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
