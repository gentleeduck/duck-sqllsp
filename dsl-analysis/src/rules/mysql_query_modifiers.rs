//! sql626: MySQL-only query modifiers / hints that have no PostgreSQL
//! equivalent and are syntax errors in PG -- `SQL_CALC_FOUND_ROWS`,
//! `STRAIGHT_JOIN`, `SQL_NO_CACHE`, `SQL_CACHE`, `HIGH_PRIORITY`,
//! `LOW_PRIORITY`, `DELAYED`. These are all word-bounded, single-token keywords
//! that PostgreSQL never uses, so they're safe to flag on sight.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// (keyword, PG guidance).
const MODS: &[(&str, &str)] = &[
  ("SQL_CALC_FOUND_ROWS", "run a separate `SELECT count(*)` (PG has no FOUND_ROWS())"),
  ("STRAIGHT_JOIN", "PG chooses join order via the planner; tune with `join_collapse_limit` if needed"),
  ("SQL_NO_CACHE", "PG has no query cache -- remove it"),
  ("SQL_CACHE", "PG has no query cache -- remove it"),
  ("HIGH_PRIORITY", "PG has no statement priorities -- remove it"),
  ("LOW_PRIORITY", "PG has no statement priorities -- remove it"),
  ("DELAYED", "PG has no INSERT DELAYED -- remove it (plain INSERT)"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql626"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    for &(kw, pg) in MODS {
      let len = kw.len();
      let mut i = 0usize;
      while i + len <= n {
        if &ub[i..i + len] == kw.as_bytes()
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + len == n || !is_word(ub[i + len] as char))
        {
          out.push(Diagnostic {
            code: "sql626",
            severity: Severity::Error,
            message: format!("`{kw}` is a MySQL-only modifier with no PostgreSQL equivalent -- {pg}"),
            range: crate::range_at(start + i, start + i + len),
          });
          i += len;
          continue;
        }
        i += 1;
      }
    }
  }
}
