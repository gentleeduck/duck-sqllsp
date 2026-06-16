//! sql625: the MySQL `ZEROFILL` column attribute (left-pads a numeric column
//! with zeros on display, and implies UNSIGNED). PostgreSQL has no display
//! attributes -- storage and presentation are separate -- so the keyword is a
//! syntax error. Format with `to_char(n, 'FM0000')` / `lpad(...)` at query time
//! instead. Sibling of the UNSIGNED lint.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql625"
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
      if &ub[i..i + 8] == b"ZEROFILL"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 8 == n || !is_word(ub[i + 8] as char))
      {
        out.push(Diagnostic {
          code: "sql625",
          severity: Severity::Error,
          message: "`ZEROFILL` is a MySQL display attribute -- PostgreSQL has none; zero-pad at query time with `to_char(n, 'FM0000')` / `lpad(...)`".into(),
          range: crate::range_at(start + i, start + i + 8),
        });
        return;
      }
      i += 1;
    }
  }
}
