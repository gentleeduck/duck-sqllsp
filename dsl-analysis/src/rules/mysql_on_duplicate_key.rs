//! sql594: `INSERT ... ON DUPLICATE KEY UPDATE ...` -- MySQL's upsert syntax.
//! PostgreSQL doesn't have it; the equivalent is
//! `INSERT ... ON CONFLICT (<conflict columns>) DO UPDATE SET ...`
//! (or `DO NOTHING`). Note PG requires you to name the conflict target.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql594"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if let Some(at) = find_phrase(upper.as_bytes(), &["ON", "DUPLICATE", "KEY"]) {
      out.push(Diagnostic {
        code: "sql594",
        severity: Severity::Error,
        message: "`ON DUPLICATE KEY UPDATE` is MySQL syntax -- PostgreSQL uses `ON CONFLICT (cols) DO UPDATE SET ...`".into(),
        range: crate::range_at(start + at, start + at + 2),
      });
    }
  }
}

fn find_phrase(ub: &[u8], words: &[&str]) -> Option<usize> {
  let n = ub.len();
  let first = words[0].as_bytes();
  let mut i = 0usize;
  while i + first.len() <= n {
    if &ub[i..i + first.len()] == first
      && (i == 0 || !is_word(ub[i - 1]))
      && matches_rest(ub, i + first.len(), &words[1..])
    {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn matches_rest(ub: &[u8], mut p: usize, words: &[&str]) -> bool {
  let n = ub.len();
  for w in words {
    while p < n && ub[p].is_ascii_whitespace() {
      p += 1;
    }
    let wb = w.as_bytes();
    if p + wb.len() <= n && &ub[p..p + wb.len()] == wb && (p + wb.len() == n || !is_word(ub[p + wb.len()])) {
      p += wb.len();
    } else {
      return false;
    }
  }
  true
}

fn is_word(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}
