//! sql653: an aggregate function inside a `CHECK` constraint, e.g.
//! `CHECK (count(*) > 0)`. A CHECK constraint is evaluated per row and cannot
//! see other rows, so PostgreSQL forbids aggregates there and raises 42803
//! ("aggregate functions are not allowed in check constraints"). Enforce
//! cross-row invariants with a trigger or a separate constraint mechanism.
//! Complements sql606 (subquery in CHECK).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const AGGREGATES: &[&str] = &[
  "count", "sum", "avg", "min", "max", "array_agg", "string_agg", "json_agg", "jsonb_agg", "bool_and",
  "bool_or", "every", "stddev", "stddev_pop", "stddev_samp", "variance", "var_pop", "var_samp", "bit_and",
  "bit_or", "corr",
];

fn close_of(b: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < b.len() {
    match b[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      }
      _ => {}
    }
    i += 1;
  }
  None
}

/// First aggregate-name + `(` occurrence in `b[lo..hi]` (lowercased), if any.
fn find_aggregate(b: &[u8], lo: usize, hi: usize) -> Option<(usize, &'static str)> {
  let mut i = lo;
  while i < hi {
    if (i == lo || !is_word(b[i - 1] as char)) && is_word(b[i] as char) {
      for &agg in AGGREGATES {
        let l = agg.len();
        if i + l <= hi && &b[i..i + l] == agg.as_bytes() && b.get(i + l).is_none_or(|&c| !is_word(c as char)) {
          let mut p = i + l;
          while p < hi && b[p].is_ascii_whitespace() {
            p += 1;
          }
          if p < hi && b[p] == b'(' {
            return Some((i, agg));
          }
        }
      }
    }
    i += 1;
  }
  None
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql653"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let lower = cleaned.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let n = lb.len();
    let mut i = 0usize;
    while i + 5 <= n {
      if &lb[i..i + 5] == b"check"
        && (i == 0 || !is_word(lb[i - 1] as char))
        && lb.get(i + 5).is_none_or(|&b| !is_word(b as char))
      {
        let mut j = i + 5;
        while j < n && lb[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n
          && lb[j] == b'('
          && let Some(c) = close_of(lb, j)
        {
          if let Some((at, agg)) = find_aggregate(lb, j + 1, c) {
            out.push(Diagnostic {
              code: "sql653",
              severity: Severity::Error,
              message: format!("aggregate `{agg}` in a CHECK constraint -- PG forbids this (42803); use a trigger for cross-row rules"),
              range: crate::range_at(start + at, start + at + agg.len()),
            });
          }
          i = c + 1;
          continue;
        }
      }
      i += 1;
    }
  }
}
