//! sql678: the MySQL "zero date" literal `'0000-00-00'` (or
//! `'0000-00-00 00:00:00'`). MySQL accepts it as a placeholder for a missing
//! date; PostgreSQL rejects it -- there is no year 0000 / month 00 / day 00,
//! so a cast raises 22008 ("date/time field value out of range"). Use NULL
//! for "no date", or a real sentinel like `'0001-01-01'`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql678"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, _upper) = crate::stmt_body_upper(stmt, source);
    let b = body.as_bytes();
    let n = b.len();

    let mut i = 0usize;
    while i < n {
      if b[i] != b'\'' {
        i += 1;
        continue;
      }
      // Opening quote of a string literal; find its close.
      let open = i;
      let mut j = i + 1;
      while j < n && b[j] != b'\'' {
        j += 1;
      }
      if open + 11 <= n && &b[open + 1..open + 11] == b"0000-00-00" {
        out.push(Diagnostic {
          code: "sql678",
          severity: Severity::Warning,
          message: "'0000-00-00' is not a valid date in PostgreSQL (raises 22008) -- use NULL or a real date".into(),
          range: crate::range_at(start + open, start + (j + 1).min(n)),
        });
      }
      i = j + 1;
    }
  }
}
