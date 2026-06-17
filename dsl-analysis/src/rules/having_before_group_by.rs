//! sql664: a `HAVING` clause that appears before `GROUP BY` at the top level.
//! The clause order is `... GROUP BY ... HAVING ...`, so `HAVING x GROUP BY y`
//! is a syntax error (42601). Move HAVING after GROUP BY. Resets at
//! set-operation boundaries so a second query's GROUP BY isn't misread.

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
    "sql664"
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
    let mut having_seen = false;
    let mut i = 0usize;
    while i < n {
      match b[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        _ if depth == 0 => {
          if kw(b, i, b"UNION") || kw(b, i, b"INTERSECT") || kw(b, i, b"EXCEPT") {
            having_seen = false;
          } else if kw(b, i, b"HAVING") {
            having_seen = true;
          } else if is_group_by(b, i) && having_seen {
            out.push(Diagnostic {
              code: "sql664",
              severity: Severity::Error,
              message: "HAVING appears before GROUP BY -- the order is GROUP BY then HAVING (PG 42601)".into(),
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
