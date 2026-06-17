//! sql648: `TABLESAMPLE SYSTEM (p)` / `TABLESAMPLE BERNOULLI (p)` where the
//! literal sampling percentage `p` is outside `0 .. 100`. PostgreSQL requires
//! the argument to be a percentage in that range and raises 22003 ("sample
//! percentage must be between 0 and 100") at run time.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql648"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let needle = b"TABLESAMPLE";
    let mut i = 0usize;
    while i + needle.len() <= n {
      if &ub[i..i + needle.len()] == needle
        && (i == 0 || !is_word(ub[i - 1] as char))
      {
        // skip method name and whitespace up to `(`
        let mut j = i + needle.len();
        while j < n && ub[j] != b'(' && ub[j] != b';' {
          j += 1;
        }
        if j < n && ub[j] == b'(' {
          let num_start = j + 1;
          let mut k = num_start;
          while k < n && ub[k] != b')' {
            k += 1;
          }
          if k < n {
            let arg = upper[num_start..k].trim();
            if let Ok(p) = arg.parse::<f64>()
              && !(0.0..=100.0).contains(&p)
            {
              out.push(Diagnostic {
                code: "sql648",
                severity: Severity::Error,
                message: format!("TABLESAMPLE percentage `{arg}` is out of range -- PG requires 0..100 (raises 22003)"),
                range: crate::range_at(start + num_start, start + k),
              });
            }
          }
          i = j;
        }
      }
      i += 1;
    }
  }
}
