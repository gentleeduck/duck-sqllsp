//! sql618: `FETCH FIRST n ROWS WITH TIES` without an `ORDER BY`. WITH TIES
//! returns the extra rows that tie with the last row *according to the ORDER
//! BY*; with no ORDER BY there's no defined ordering, so PostgreSQL rejects it
//! (42601, "WITH TIES cannot be specified without ORDER BY clause"). Add an
//! ORDER BY, or use plain `ROWS ONLY` / `LIMIT`.
//!
//! Conservative: only flags when the statement has no ORDER BY at all.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql618"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    // normalise internal whitespace runs so "WITH   TIES" still matches
    let Some(at) = find_phrase(&upper, &["WITH", "TIES"]) else {
      return;
    };
    if upper.contains("ORDER BY") {
      return;
    }
    out.push(Diagnostic {
      code: "sql618",
      severity: Severity::Error,
      message: "WITH TIES requires an ORDER BY -- PostgreSQL raises 42601; add ORDER BY or use ROWS ONLY / LIMIT".into(),
      range: crate::range_at(start + at, start + at + 4),
    });
  }
}

/// Byte index of the start of `words[0]` for the first place where the words
/// appear consecutively (whitespace-separated), each on a word boundary.
fn find_phrase(upper: &str, words: &[&str]) -> Option<usize> {
  let b = upper.as_bytes();
  let n = b.len();
  let first = words[0].as_bytes();
  let mut i = 0usize;
  'outer: while i + first.len() <= n {
    if &b[i..i + first.len()] == first
      && (i == 0 || !(b[i - 1] as char).is_alphanumeric() && b[i - 1] != b'_')
    {
      let mut j = i + first.len();
      for w in &words[1..] {
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
      // trailing boundary
      if j == n || !(b[j] as char).is_alphanumeric() && b[j] != b'_' {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
