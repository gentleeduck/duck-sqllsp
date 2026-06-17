//! sql610: `SELECT ... OVER (...) ... FOR UPDATE` -- PostgreSQL raises 0A000
//! "FOR UPDATE is not allowed with window functions" at parse time. A window
//! function computes over a frame of rows, so there's no single base row to
//! lock. Drop the lock, or compute the window in a subquery and lock the outer
//! plain query.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

const LOCKS: &[&str] = &["FOR UPDATE", "FOR NO KEY UPDATE", "FOR SHARE", "FOR KEY SHARE"];

/// True if `upper` contains a window-function `OVER (` / `OVER(`.
fn has_window(upper: &str) -> bool {
  let ub = upper.as_bytes();
  let n = ub.len();
  let mut i = 0usize;
  while i + 4 <= n {
    if &ub[i..i + 4] == b"OVER"
      && (i == 0 || !is_word(ub[i - 1] as char))
      && ub.get(i + 4).is_none_or(|&b| !is_word(b as char))
    {
      let mut j = i + 4;
      while j < n && ub[j].is_ascii_whitespace() {
        j += 1;
      }
      if j < n && ub[j] == b'(' {
        return true;
      }
    }
    i += 1;
  }
  false
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql610"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(_) = &stmt.kind else { return };
    let (start, raw) = crate::stmt_body(stmt, source);
    let body = crate::textutil::strip_noise_full(raw);
    let upper = body.to_ascii_uppercase();
    let Some(at) = LOCKS.iter().find_map(|l| upper.find(l)) else {
      return;
    };
    if has_window(&upper) {
      out.push(Diagnostic {
        code: "sql610",
        severity: Severity::Error,
        message: "FOR UPDATE/SHARE is not allowed with window functions -- PG raises 0A000; lock the outer query over a windowed subquery".into(),
        range: crate::range_at(start + at, start + at + upper[at..].find([';', '\n']).unwrap_or(upper.len() - at)),
      });
    }
  }
}
