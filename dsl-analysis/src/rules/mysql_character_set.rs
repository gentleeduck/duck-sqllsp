//! sql616: a MySQL `CHARACTER SET ...` / `CHARSET=...` clause (per-column or
//! per-table). PostgreSQL has no per-column or per-table character sets -- the
//! encoding is fixed per database -- so these clauses are syntax errors (42601).
//! Drop them; use `COLLATE "..."` for collation, and set the encoding at
//! `CREATE DATABASE ... ENCODING` time.
//!
//! `CHARACTER SET` is matched as a two-word phrase so it never collides with
//! `CHARACTER VARYING` (a valid spelling of `varchar`).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql616"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    // CHARSET (the MySQL shorthand, usually `CHARSET=utf8`)
    let mut i = 0usize;
    while i + 7 <= n {
      if &ub[i..i + 7] == b"CHARSET"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub.get(i + 7).is_none_or(|&b| !is_word(b as char))
      {
        out.push(Diagnostic {
          code: "sql616",
          severity: Severity::Error,
          message: "`CHARSET` is MySQL syntax -- PostgreSQL has no per-column/table character sets; use `COLLATE` or set the database encoding".into(),
          range: crate::range_at(start + i, start + i + 7),
        });
        return;
      }
      i += 1;
    }

    // CHARACTER SET (two words; avoids CHARACTER VARYING)
    let mut i = 0usize;
    while i + 9 <= n {
      if &ub[i..i + 9] == b"CHARACTER"
        && (i == 0 || !is_word(ub[i - 1] as char))
      {
        let mut j = i + 9;
        let ws_start = j;
        while j < n && ub[j].is_ascii_whitespace() {
          j += 1;
        }
        if j > ws_start
          && j + 3 <= n
          && &ub[j..j + 3] == b"SET"
          && ub.get(j + 3).is_none_or(|&b| !is_word(b as char))
        {
          out.push(Diagnostic {
            code: "sql616",
            severity: Severity::Error,
            message: "`CHARACTER SET` is MySQL syntax -- PostgreSQL has no per-column/table character sets; use `COLLATE` or set the database encoding".into(),
            range: crate::range_at(start + i, start + j + 3),
          });
          return;
        }
      }
      i += 1;
    }
  }
}
