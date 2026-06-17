//! sql661: a window-only function (`row_number`, `rank`, `dense_rank`,
//! `lag`, `lead`, `ntile`, `first_value`, ...) called without an `OVER` clause.
//! These functions exist only as window functions, so PostgreSQL raises 42P20
//! ("window function ... requires an OVER clause"). Add `OVER (...)` (with the
//! appropriate PARTITION BY / ORDER BY).
//!
//! Unlike `count`/`sum`/`min`/..., these names have no aggregate meaning, so a
//! missing OVER is always an error.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const WINDOW_ONLY: &[&str] = &[
  "row_number",
  "dense_rank",
  "percent_rank",
  "cume_dist",
  "first_value",
  "last_value",
  "nth_value",
  "ntile",
  "rank",
  "lag",
  "lead",
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

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql661"
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
    for &fname in WINDOW_ONLY {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(fname) {
        let at = from + rel;
        from = at + fname.len();
        // word boundary before, `(` (after optional ws) after
        if at > 0 && is_word(lb[at - 1] as char) {
          continue;
        }
        let mut p = at + fname.len();
        while p < n && lb[p].is_ascii_whitespace() {
          p += 1;
        }
        if p >= n || lb[p] != b'(' {
          continue;
        }
        let Some(close) = close_of(lb, p) else { continue };
        // after the args, an OVER clause must follow
        let mut q = close + 1;
        while q < n && lb[q].is_ascii_whitespace() {
          q += 1;
        }
        let has_over = q + 4 <= n && &lb[q..q + 4] == b"over" && lb.get(q + 4).is_none_or(|&c| !is_word(c as char));
        if !has_over {
          out.push(Diagnostic {
            code: "sql661",
            severity: Severity::Error,
            message: format!("`{fname}` is a window function and requires an OVER clause -- add `OVER (...)` (PG 42P20)"),
            range: crate::range_at(start + at, start + at + fname.len()),
          });
        }
      }
    }
  }
}
