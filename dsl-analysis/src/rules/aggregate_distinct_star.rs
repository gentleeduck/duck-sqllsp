//! sql646: `count(DISTINCT *)` (or any aggregate with `DISTINCT *`).
//! PostgreSQL doesn't support `DISTINCT *` inside an aggregate and raises a
//! syntax error -- `count(DISTINCT col)` needs an explicit column (or list of
//! columns), and `count(*)` already counts every row. This is a frequent
//! mistranslation of "count the distinct rows".

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql646"
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
      if &ub[i..i + 8] == b"DISTINCT"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub.get(i + 8).is_none_or(|&b| !is_word(b as char))
      {
        // forward: whitespace then `*`
        let mut f = i + 8;
        while f < n && ub[f].is_ascii_whitespace() {
          f += 1;
        }
        // backward: whitespace then `(` (aggregate argument list)
        let mut b = i;
        while b > 0 && ub[b - 1].is_ascii_whitespace() {
          b -= 1;
        }
        if f < n && ub[f] == b'*' && b > 0 && ub[b - 1] == b'(' {
          out.push(Diagnostic {
            code: "sql646",
            severity: Severity::Error,
            message: "`DISTINCT *` is not allowed in an aggregate -- use `count(*)` for all rows or `count(DISTINCT col)` for distinct values".into(),
            range: crate::range_at(start + i, start + f + 1),
          });
          return;
        }
      }
      i += 1;
    }
  }
}
