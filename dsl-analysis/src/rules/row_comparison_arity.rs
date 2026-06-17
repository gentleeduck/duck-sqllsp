//! sql650: a row-constructor comparison with unequal arity, e.g.
//! `(a, b) = (1, 2, 3)`. Comparing two row constructors requires the same
//! number of fields on each side; PostgreSQL raises 42601 ("unequal number of
//! entries in row expressions").
//!
//! Conservative: only bare parenthesised lists (not `func(...)` calls) on both
//! sides of `=` / `<>` are considered, and each side must contain a top-level
//! comma (so it's genuinely a row, not a parenthesised scalar).

use crate::clause_scan::{is_word, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// Matching close paren index for the `(` at `open`.
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

/// Matching open paren index for the `)` at `close`.
fn open_of(b: &[u8], close: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = close as isize;
  while i >= 0 {
    match b[i as usize] {
      b')' => depth += 1,
      b'(' => {
        depth -= 1;
        if depth == 0 {
          return Some(i as usize);
        }
      }
      _ => {}
    }
    i -= 1;
  }
  None
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql650"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let b = cleaned.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    while i < n {
      // comparison operator: `=` (not `<=`/`>=`/`!=`/`<>`) or `<>`
      let is_eq = b[i] == b'=' && b.get(i.wrapping_sub(1)).is_none_or(|&c| !matches!(c, b'<' | b'>' | b'!' | b'='));
      let is_neq = b[i] == b'<' && b.get(i + 1) == Some(&b'>');
      if is_eq || is_neq {
        let op_len = if is_neq { 2 } else { 1 };
        // left side must end with `)`
        let mut l = i;
        while l > 0 && b[l - 1].is_ascii_whitespace() {
          l -= 1;
        }
        // right side must begin with `(`
        let mut r = i + op_len;
        while r < n && b[r].is_ascii_whitespace() {
          r += 1;
        }
        if l > 0
          && b[l - 1] == b')'
          && r < n
          && b[r] == b'('
          && let (Some(lopen), Some(rclose)) = (open_of(b, l - 1), close_of(b, r))
        {
          {
            // both must be bare parens (not function calls)
            let left_bare = lopen == 0 || !is_word(b[lopen - 1] as char);
            let right_bare = r == 0 || !is_word(b[r - 1] as char);
            let lcols = split_top_level(&cleaned[lopen + 1..l - 1]);
            let rcols = split_top_level(&cleaned[r + 1..rclose]);
            if left_bare && right_bare && lcols.len() > 1 && rcols.len() > 1 && lcols.len() != rcols.len() {
              out.push(Diagnostic {
                code: "sql650",
                severity: Severity::Error,
                message: format!("row comparison has unequal arity ({} vs {}) -- PG raises 42601", lcols.len(), rcols.len()),
                range: crate::range_at(start + lopen, start + rclose + 1),
              });
            }
          }
        }
      }
      i += 1;
    }
  }
}
