//! sql719: `CREATE SEQUENCE ... INCREMENT 0` (or `INCREMENT BY 0`, also in
//! `ALTER SEQUENCE`) -- the increment must be non-zero. PostgreSQL rejects it
//! with 22023 ("INCREMENT must not be zero"). Usually a placeholder or a
//! variable that resolved to 0.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql719"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    // Only relevant inside sequence DDL.
    if !contains_word(ub, b"SEQUENCE") {
      return;
    }

    let mut i = 0usize;
    while i < n {
      if !word_at(ub, i, b"INCREMENT") {
        i += 1;
        continue;
      }
      let mut p = skip_ws(ub, i + 9);
      if word_at(ub, p, b"BY") {
        p = skip_ws(ub, p + 2);
      }
      // Optional sign, then a literal that is exactly 0.
      if is_zero(ub, p) {
        out.push(Diagnostic {
          code: "sql719",
          severity: Severity::Warning,
          message: "sequence INCREMENT must not be zero (PG 22023)".into(),
          range: crate::range_at(start + p, start + p + 1),
        });
      }
      i += 9;
    }
  }
}

/// `0` as a complete integer literal at `i` (not `0.5`, `00`, ...).
fn is_zero(ub: &[u8], i: usize) -> bool {
  ub.get(i) == Some(&b'0') && !matches!(ub.get(i + 1), Some(c) if c.is_ascii_digit() || *c == b'.' || is_word(*c as char))
}

fn contains_word(ub: &[u8], w: &[u8]) -> bool {
  let mut i = 0usize;
  while i + w.len() <= ub.len() {
    if word_at(ub, i, w) {
      return true;
    }
    i += 1;
  }
  false
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
