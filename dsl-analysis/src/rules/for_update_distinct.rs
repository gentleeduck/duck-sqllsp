//! sql609: `SELECT DISTINCT ... FOR UPDATE` -- PostgreSQL raises 0A000
//! "FOR UPDATE is not allowed with DISTINCT clause" at parse time. Row locking
//! needs a plain row source; a DISTINCT (which collapses rows) has no single
//! row to lock. Drop DISTINCT, or lock in a separate query over the base table.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

const LOCKS: &[&str] = &["FOR UPDATE", "FOR NO KEY UPDATE", "FOR SHARE", "FOR KEY SHARE"];

/// True if `upper` contains a select-level `SELECT DISTINCT` (whitespace
/// between the two words allowed), as opposed to an aggregate `count(DISTINCT)`.
fn has_select_distinct(upper: &str) -> bool {
  let ub = upper.as_bytes();
  let n = ub.len();
  let mut i = 0usize;
  while i + 8 <= n {
    if &ub[i..i + 8] == b"DISTINCT"
      && (i == 0 || !is_word(ub[i - 1] as char))
      && ub.get(i + 8).is_none_or(|&b| !is_word(b as char))
    {
      // walk back over whitespace, then require the word `SELECT`
      let mut j = i;
      while j > 0 && ub[j - 1].is_ascii_whitespace() {
        j -= 1;
      }
      if j >= 6
        && &ub[j - 6..j] == b"SELECT"
        && (j == 6 || !is_word(ub[j - 7] as char))
      {
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
    "sql609"
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
    if has_select_distinct(&upper) {
      out.push(Diagnostic {
        code: "sql609",
        severity: Severity::Error,
        message: "FOR UPDATE/SHARE is not allowed with DISTINCT -- PG raises 0A000; lock a plain row source instead".into(),
        range: crate::range_at(start + at, start + at + upper[at..].find([';', '\n']).unwrap_or(upper.len() - at)),
      });
    }
  }
}
