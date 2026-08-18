//! sql798: a bare `LOOP ... END LOOP` (not FOR/WHILE) whose body
//! contains no `EXIT`, `RETURN`, or `RAISE` anywhere -- guaranteed
//! infinite loop.

use crate::clause_scan::is_word;
use crate::textutil::contains_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const ESCAPE_WORDS: &[&str] = &["EXIT", "RETURN", "RAISE"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql798"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"LOOP") {
        i += 1;
        continue;
      }
      if !is_bare_loop(ub, i) {
        i += 4;
        continue;
      }
      let Some(end_loop) = matching_end_loop(ub, i) else {
        i += 4;
        continue;
      };
      let body_span = &upper[i + 4..end_loop];
      if !ESCAPE_WORDS.iter().any(|w| contains_word(body_span, w)) {
        out.push(Diagnostic {
          code: "sql798",
          severity: Severity::Warning,
          message: "LOOP body has no EXIT, RETURN, or RAISE -- guaranteed infinite loop".into(),
          range: crate::range_at(start + i, start + i + 4),
        });
      }
      i = end_loop + 3;
    }
  }
}

/// True when `loop_pos` is a bare LOOP: scanning back to the nearest
/// `;` (or buffer start), there's no FOR or WHILE keyword in between
/// (which would make this a FOR/WHILE loop header instead).
fn is_bare_loop(ub: &[u8], loop_pos: usize) -> bool {
  let mut i = loop_pos;
  while i > 0 && ub[i - 1] != b';' {
    i -= 1;
  }
  let seg = &ub[i..loop_pos];
  !contains_word_bytes(seg, b"FOR") && !contains_word_bytes(seg, b"WHILE")
}

/// The matching `END LOOP` for the `LOOP` at `loop_pos`, tracking
/// nesting depth across every `LOOP`/`END LOOP` pair regardless of
/// loop kind (bare/FOR/WHILE all close with `END LOOP`). Returns the
/// offset of the matching `END`.
fn matching_end_loop(ub: &[u8], loop_pos: usize) -> Option<usize> {
  let mut depth = 1i32;
  let mut i = loop_pos + 4;
  while i < ub.len() {
    if !word_at(ub, i, b"LOOP") {
      i += 1;
      continue;
    }
    if let Some(end_pos) = end_immediately_before(ub, i) {
      depth -= 1;
      if depth == 0 {
        return Some(end_pos);
      }
    } else {
      depth += 1;
    }
    i += 4;
  }
  None
}

/// If the word immediately before `pos` (skipping whitespace) is
/// `END`, return its start offset.
fn end_immediately_before(ub: &[u8], pos: usize) -> Option<usize> {
  let mut e = pos;
  while e > 0 && ub[e - 1].is_ascii_whitespace() {
    e -= 1;
  }
  if e >= 3 && &ub[e - 3..e] == b"END" && (e == 3 || !is_word(ub[e - 4] as char)) {
    return Some(e - 3);
  }
  None
}

fn contains_word_bytes(hay: &[u8], w: &[u8]) -> bool {
  let mut i = 0usize;
  while i + w.len() <= hay.len() {
    if word_at(hay, i, w) {
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
