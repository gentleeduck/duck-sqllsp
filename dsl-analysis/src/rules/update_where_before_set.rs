//! sql665: an `UPDATE` whose `WHERE` clause comes before `SET`, e.g.
//! `UPDATE t WHERE id = 1 SET x = 2`. The required order is
//! `UPDATE t SET ... WHERE ...`; writing WHERE first is a syntax error (42601).
//! Depth-0 only, so a `WHERE` inside a `SET x = (SELECT ... WHERE ...)` subquery
//! is fine.

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
    "sql665"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    if !upper.trim_start().starts_with("UPDATE") {
      return;
    }
    let b = upper.as_bytes();
    let n = b.len();
    let mut depth = 0i32;
    let mut set_seen = false;
    let mut i = 0usize;
    while i < n {
      match b[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        _ if depth == 0 => {
          if kw(b, i, b"SET") {
            set_seen = true;
          } else if kw(b, i, b"WHERE") && !set_seen {
            out.push(Diagnostic {
              code: "sql665",
              severity: Severity::Error,
              message: "WHERE comes before SET in this UPDATE -- the order is `UPDATE t SET ... WHERE ...` (PG 42601)".into(),
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
