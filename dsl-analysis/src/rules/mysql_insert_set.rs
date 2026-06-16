//! sql667: MySQL's `INSERT INTO t SET a = 1, b = 2` assignment-list syntax.
//! PostgreSQL's INSERT uses a column list and `VALUES` (or a `SELECT`):
//! `INSERT INTO t (a, b) VALUES (1, 2)`. The `SET` form is a syntax error in PG.
//!
//! Only a `SET` reached before any `VALUES` / `SELECT` / `ON CONFLICT` is
//! flagged, so the legitimate `... ON CONFLICT ... DO UPDATE SET ...` is left
//! alone.

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
    "sql667"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    if !upper.trim_start().starts_with("INSERT") {
      return;
    }
    let b = upper.as_bytes();
    let n = b.len();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < n {
      match b[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        _ if depth == 0 => {
          if kw(b, i, b"SET") {
            out.push(Diagnostic {
              code: "sql667",
              severity: Severity::Error,
              message: "`INSERT ... SET` is MySQL syntax -- PostgreSQL uses `INSERT INTO t (cols) VALUES (...)`".into(),
              range: crate::range_at(start + i, start + i + 3),
            });
            return;
          }
          if kw(b, i, b"VALUES") || kw(b, i, b"SELECT") || kw(b, i, b"CONFLICT") {
            return;
          }
        }
        _ => {}
      }
      i += 1;
    }
  }
}
