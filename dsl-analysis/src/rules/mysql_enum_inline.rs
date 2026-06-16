//! sql623: a MySQL inline `ENUM('a','b',...)` column type. PostgreSQL has no
//! inline enum: you declare a named type with `CREATE TYPE x AS ENUM (...)` and
//! reference it, or model the constraint with `CHECK (col IN ('a','b'))`. The
//! inline form is a syntax error in PG (42601).
//!
//! `ENUM(` is matched word-bounded; PostgreSQL never uses `ENUM(...)` as an
//! expression, so there's nothing legitimate to confuse it with.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql623"
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
      if &ub[i..i + 4] == b"ENUM"
        && (i == 0 || !is_word(ub[i - 1] as char))
      {
        let mut j = i + 4;
        while j < n && ub[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && ub[j] == b'(' {
          out.push(Diagnostic {
            code: "sql623",
            severity: Severity::Error,
            message: "inline `ENUM(...)` is MySQL syntax -- PostgreSQL has no inline enum; use `CREATE TYPE ... AS ENUM (...)` and reference it, or a CHECK constraint".into(),
            range: crate::range_at(start + i, start + i + 4),
          });
          i = j;
          continue;
        }
      }
      i += 1;
    }
  }
}
