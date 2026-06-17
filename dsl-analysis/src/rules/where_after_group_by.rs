//! sql659: a `WHERE` clause that appears after `GROUP BY` at the top level of a
//! query. SQL fixes the order as `... WHERE ... GROUP BY ... HAVING ...`, so
//! `GROUP BY a WHERE b` is a syntax error (42601) -- the row filter belongs
//! before GROUP BY (or, for post-aggregation filtering, use HAVING).
//!
//! Resets at set-operation boundaries, so `... GROUP BY a UNION SELECT ...
//! WHERE b` (a second query's WHERE) is not flagged.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

fn kw(b: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= b.len()
    && &b[i..i + w.len()] == w
    && (i == 0 || !is_word(b[i - 1] as char))
    && b.get(i + w.len()).is_none_or(|&c| !is_word(c as char))
}

fn is_group_by(b: &[u8], i: usize) -> bool {
  if !kw(b, i, b"GROUP") {
    return false;
  }
  let mut j = i + 5;
  if j >= b.len() || !b[j].is_ascii_whitespace() {
    return false;
  }
  while j < b.len() && b[j].is_ascii_whitespace() {
    j += 1;
  }
  kw(b, j, b"BY")
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql659"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let b = upper.as_bytes();
    let n = b.len();
    let mut depth = 0i32;
    let mut group_seen = false;
    let mut i = 0usize;
    while i < n {
      match b[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        _ if depth == 0 => {
          if kw(b, i, b"UNION") || kw(b, i, b"INTERSECT") || kw(b, i, b"EXCEPT") {
            group_seen = false;
          } else if is_group_by(b, i) {
            group_seen = true;
            i += 5;
            continue;
          } else if kw(b, i, b"WHERE") && group_seen {
            out.push(Diagnostic {
              code: "sql659",
              severity: Severity::Error,
              message: "WHERE appears after GROUP BY -- WHERE must precede GROUP BY (use HAVING to filter after aggregation) (PG 42601)".into(),
              range: crate::range_at(start + i, start + i + 5),
            });
            return;
          }
        }
        _ => {}
      }
      i += 1;
    }
  }
}
