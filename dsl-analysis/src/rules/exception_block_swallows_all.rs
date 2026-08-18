//! sql800: `EXCEPTION WHEN OTHERS THEN` with an empty or `NULL;`-only
//! body -- silently discards every error. Classic PL/pgSQL
//! anti-pattern.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql800"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"WHEN") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 4);
      if !word_at(ub, p, b"OTHERS") {
        i += 4;
        continue;
      }
      let q = skip_ws(ub, p + 6);
      if !word_at(ub, q, b"THEN") {
        i += 4;
        continue;
      }
      let handler_start = q + 4;
      let handler_end = find_handler_end(ub, handler_start);
      let handler = upper[handler_start..handler_end].trim();
      let is_empty = handler.is_empty();
      let is_null_only = handler.trim_end_matches(';').trim() == "NULL";
      if is_empty || is_null_only {
        out.push(Diagnostic {
          code: "sql800",
          severity: Severity::Warning,
          message: "EXCEPTION WHEN OTHERS THEN with an empty/NULL-only body silently discards every error".into(),
          range: crate::range_at(start + i, start + q + 4),
        });
      }
      i = handler_end;
    }
  }
}

/// Find the end of this exception handler's body -- the next
/// top-level `WHEN` (another handler) or `END` (closing the block),
/// whichever comes first, tracking nested BEGIN/END depth.
fn find_handler_end(ub: &[u8], from: usize) -> usize {
  let mut depth = 0i32;
  let mut i = from;
  while i < ub.len() {
    if word_at(ub, i, b"BEGIN") {
      depth += 1;
      i += 5;
      continue;
    }
    if word_at(ub, i, b"END") {
      if depth == 0 {
        return i;
      }
      depth -= 1;
      i += 3;
      continue;
    }
    if depth == 0 && word_at(ub, i, b"WHEN") {
      return i;
    }
    i += 1;
  }
  ub.len()
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
