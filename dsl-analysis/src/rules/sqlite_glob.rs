//! sql637: the SQLite `GLOB` operator (case-sensitive, Unix-glob pattern match,
//! e.g. `name GLOB 'foo*'`). PostgreSQL has no GLOB operator. Use `LIKE` with
//! `%`/`_` wildcards (case-sensitive by default), or the POSIX regex operator
//! `~` / `~*`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql637"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 4 <= n {
      // infix-operator usage: whitespace on both sides
      if &ub[i..i + 4] == b"GLOB"
        && i > 0
        && ub[i - 1].is_ascii_whitespace()
        && i + 4 < n
        && ub[i + 4].is_ascii_whitespace()
        && !is_word(ub[i - 1] as char)
      {
        out.push(Diagnostic {
          code: "sql637",
          severity: Severity::Error,
          message: "`GLOB` is a SQLite operator -- PostgreSQL uses `LIKE` (wildcards `%`/`_`) or the regex operators `~` / `~*`".into(),
          range: crate::range_at(start + i, start + i + 4),
        });
        i += 4;
        continue;
      }
      i += 1;
    }
  }
}
