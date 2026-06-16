//! sql593: `LIMIT 10, 20` -- MySQL's `LIMIT offset, count` syntax. PostgreSQL
//! doesn't accept it and raises a syntax error; the equivalent is
//! `LIMIT 20 OFFSET 10` (note the order flips -- count first). A common slip
//! when porting MySQL queries.

use crate::clause_scan::{find_clause, find_clause_end};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql593"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(at) = find_clause(ub, b"LIMIT") else { return };
    let ls = at + 5;
    let le = find_clause_end(ub, ls, &["OFFSET", "FOR", "FETCH"]);
    // A top-level comma in the LIMIT clause is the MySQL `offset, count` form.
    let bytes = body.as_bytes();
    let mut depth = 0i32;
    let mut i = ls;
    while i < le {
      match bytes[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        b'\'' => {
          i += 1;
          while i < le && bytes[i] != b'\'' {
            i += 1;
          }
        },
        b',' if depth == 0 => {
          out.push(Diagnostic {
            code: "sql593",
            severity: Severity::Error,
            message: "`LIMIT offset, count` is MySQL syntax -- PostgreSQL uses `LIMIT count OFFSET offset`".into(),
            range: crate::range_at(start + at, start + le),
          });
          return;
        },
        _ => {},
      }
      i += 1;
    }
  }
}
