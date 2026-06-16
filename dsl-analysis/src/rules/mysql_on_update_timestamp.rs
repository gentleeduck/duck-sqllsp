//! sql624: the MySQL column attribute `ON UPDATE CURRENT_TIMESTAMP` (auto-touch
//! a timestamp column on every row update). PostgreSQL has no such column
//! attribute and rejects it; implement it with a `BEFORE UPDATE` trigger that
//! sets the column to `now()`.
//!
//! Only `ON UPDATE` immediately followed by a current-time function is flagged,
//! so foreign-key actions (`ON UPDATE CASCADE` / `SET NULL` / `RESTRICT` /
//! `NO ACTION`) are left alone.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const NOW_FNS: &[&str] = &["CURRENT_TIMESTAMP", "LOCALTIMESTAMP", "NOW"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql624"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 2 <= n {
      // match `ON` then ws then `UPDATE`
      if &ub[i..i + 2] == b"ON"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub.get(i + 2).is_some_and(|&b| (b as char).is_ascii_whitespace())
      {
        let mut j = i + 2;
        while j < n && ub[j].is_ascii_whitespace() {
          j += 1;
        }
        if j + 6 <= n && &ub[j..j + 6] == b"UPDATE" && ub.get(j + 6).is_none_or(|&b| !is_word(b as char)) {
          let mut k = j + 6;
          while k < n && ub[k].is_ascii_whitespace() {
            k += 1;
          }
          for &f in NOW_FNS {
            let l = f.len();
            if k + l <= n && &ub[k..k + l] == f.as_bytes() && ub.get(k + l).is_none_or(|&b| !is_word(b as char)) {
              out.push(Diagnostic {
                code: "sql624",
                severity: Severity::Error,
                message: "`ON UPDATE CURRENT_TIMESTAMP` is a MySQL column attribute -- PostgreSQL has no auto-update timestamp; use a BEFORE UPDATE trigger that sets the column to now()".into(),
                range: crate::range_at(start + i, start + k + l),
              });
              return;
            }
          }
        }
      }
      i += 1;
    }
  }
}
