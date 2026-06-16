//! sql669: MySQL's `SELECT ... LOCK IN SHARE MODE` row-locking clause.
//! PostgreSQL spells a shared row lock `FOR SHARE` (and an exclusive one
//! `FOR UPDATE`). `LOCK IN SHARE MODE` is a syntax error in PG.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// Find a whitespace-separated phrase, returning the start index.
fn find_phrase(upper: &str, words: &[&str]) -> Option<usize> {
  let b = upper.as_bytes();
  let n = b.len();
  let first = words[0].as_bytes();
  let is_w = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
  let mut i = 0usize;
  'outer: while i + first.len() <= n {
    if &b[i..i + first.len()] == first && (i == 0 || !is_w(b[i - 1])) {
      let mut j = i + first.len();
      for w in &words[1..] {
        if j >= n || !b[j].is_ascii_whitespace() {
          i += 1;
          continue 'outer;
        }
        while j < n && b[j].is_ascii_whitespace() {
          j += 1;
        }
        let wb = w.as_bytes();
        if j + wb.len() > n || &b[j..j + wb.len()] != wb {
          i += 1;
          continue 'outer;
        }
        j += wb.len();
      }
      if j == n || !is_w(b[j]) {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql669"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if let Some(at) = find_phrase(&upper, &["LOCK", "IN", "SHARE", "MODE"]) {
      let end = at + "LOCK IN SHARE MODE".len();
      out.push(Diagnostic {
        code: "sql669",
        severity: Severity::Error,
        message: "`LOCK IN SHARE MODE` is MySQL -- PostgreSQL uses `FOR SHARE` (or `FOR UPDATE` for an exclusive lock)".into(),
        range: crate::range_at(start + at, start + end.min(upper.len())),
      });
    }
  }
}
