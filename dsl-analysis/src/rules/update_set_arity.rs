//! sql655: a multi-column UPDATE assignment whose column list and value list
//! have different lengths, e.g. `UPDATE t SET (a, b) = (1, 2, 3)`. PostgreSQL
//! requires the two lists to match and raises 42601 ("number of columns does
//! not match number of values").
//!
//! Only a literal value list `(...)` is checked; a sub-SELECT value source
//! (`SET (a, b) = (SELECT ...)`) is skipped.

use crate::clause_scan::{is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

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
    "sql655"
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
    let ub = upper.as_bytes();
    let n = ub.len();
    // locate the SET keyword
    let mut i = 0usize;
    while i + 3 <= n {
      if &ub[i..i + 3] == b"SET"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && ub.get(i + 3).is_some_and(|&b| b.is_ascii_whitespace())
      {
        break;
      }
      i += 1;
    }
    if i + 3 >= n {
      return;
    }
    let mut j = i + 3;
    while j < n && ub[j].is_ascii_whitespace() {
      j += 1;
    }
    if j >= n || ub[j] != b'(' {
      return; // single-column assignment
    }
    let Some(lclose) = close_of(ub, j) else { return };
    let mut k = lclose + 1;
    while k < n && ub[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= n || ub[k] != b'=' {
      return;
    }
    k += 1;
    while k < n && ub[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= n || ub[k] != b'(' {
      return;
    }
    let Some(rclose) = close_of(ub, k) else { return };
    let rhs = &cleaned[k + 1..rclose];
    if rhs.trim_start().to_ascii_uppercase().starts_with("SELECT") {
      return; // sub-SELECT value source
    }
    let lcols = split_top_level(&cleaned[j + 1..lclose]).len();
    let rcols = split_top_level(rhs).len();
    if lcols != rcols {
      out.push(Diagnostic {
        code: "sql655",
        severity: Severity::Error,
        message: format!("UPDATE assigns {rcols} values to {lcols} columns -- PG raises 42601; the column and value lists must match"),
        range: crate::range_at(start + j, start + rclose + 1),
      });
    }
  }
}
