//! sql582: the `money` column type. It carries a fixed, locale-dependent
//! fractional precision, its text output depends on `lc_monetary`, and
//! arithmetic with it is awkward (no clean multiply/divide by fractions).
//! Store currency as `numeric(p, s)` (or integer minor units) instead.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql582"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Only meaningful in a type position -- scope to DDL to avoid columns or
    // aliases that happen to be called `money`.
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !(upper.contains("CREATE TABLE") || upper.contains("ALTER TABLE") || upper.contains("CREATE TYPE") || upper.contains("::MONEY")) {
      return;
    }
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i < n {
      match ub[i] {
        b'\'' => {
          i += 1;
          while i < n && ub[i] != b'\'' {
            i += 1;
          }
        },
        b'M' if i + 5 <= n && &ub[i..i + 5] == b"MONEY" => {
          if (i == 0 || !is_word(ub[i - 1] as char)) && ub.get(i + 5).is_none_or(|&b| !is_word(b as char) && b != b'(') {
            out.push(Diagnostic {
              code: "sql582",
              severity: Severity::Hint,
              message: "the `money` type has locale-dependent formatting and rounding quirks -- use `numeric` for currency".into(),
              range: crate::range_at(start + i, start + i + 5),
            });
            return;
          }
          i += 5;
          continue;
        },
        _ => {},
      }
      i += 1;
    }
  }
}
