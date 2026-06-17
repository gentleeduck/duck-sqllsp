//! sql656: a `TRUNCATE` statement with a `WHERE` clause. TRUNCATE removes *all*
//! rows of a table and accepts no row filter -- `TRUNCATE t WHERE ...` is a
//! syntax error (42601). The mistake usually means a conditional `DELETE FROM t
//! WHERE ...` was intended (TRUNCATE can't be filtered).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql656"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    if !upper.trim_start().starts_with("TRUNCATE") {
      return;
    }
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i + 5 <= n {
      match ub[i] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        _ if depth == 0
          && &ub[i..i + 5] == b"WHERE"
          && (i == 0 || !is_word(ub[i - 1] as char))
          && ub.get(i + 5).is_none_or(|&b| !is_word(b as char)) =>
        {
          out.push(Diagnostic {
            code: "sql656",
            severity: Severity::Error,
            message: "TRUNCATE does not take a WHERE clause -- it removes all rows; use `DELETE FROM ... WHERE ...` for a conditional delete (PG 42601)".into(),
            range: crate::range_at(start + i, start + i + 5),
          });
          return;
        }
        _ => {}
      }
      i += 1;
    }
  }
}
