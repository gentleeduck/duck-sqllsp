//! sql658: both a `LIMIT` clause and a `FETCH FIRST/NEXT ... ROWS` clause in the
//! same query level. They're two spellings of the same row-limit, and
//! PostgreSQL allows only one -- specifying both raises 42601 ("multiple LIMIT
//! options not allowed"). Keep one.
//!
//! Depth-0 only, so a subquery's LIMIT and the outer query's FETCH don't
//! collide.

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

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql658"
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
    let mut limit_at: Option<usize> = None;
    let mut fetch_at: Option<usize> = None;
    let mut i = 0usize;
    while i < n {
      match b[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        _ if depth == 0 => {
          if limit_at.is_none() && kw(b, i, b"LIMIT") {
            limit_at = Some(i);
          } else if fetch_at.is_none() && kw(b, i, b"FETCH") {
            // require the row-limit form: FETCH FIRST / FETCH NEXT
            let mut j = i + 5;
            while j < n && b[j].is_ascii_whitespace() {
              j += 1;
            }
            if kw(b, j, b"FIRST") || kw(b, j, b"NEXT") {
              fetch_at = Some(i);
            }
          }
        }
        _ => {}
      }
      i += 1;
    }
    if let (Some(l), Some(f)) = (limit_at, fetch_at) {
      let at = l.max(f);
      out.push(Diagnostic {
        code: "sql658",
        severity: Severity::Error,
        message: "both LIMIT and FETCH FIRST specified -- PG allows only one row-limit clause (42601); keep one".into(),
        range: crate::range_at(start + at, start + at + 5),
      });
    }
  }
}
