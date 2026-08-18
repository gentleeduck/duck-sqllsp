//! sql803: `RAISE NOTICE` appears inside a loop body (bare `LOOP`,
//! `FOR ... LOOP`, or `WHILE ... LOOP`) -- a per-iteration notice on a
//! bulk operation is a common, easy-to-miss log-noise/performance
//! footgun. Flags any RAISE NOTICE inside a loop body regardless of
//! further conditional nesting inside that loop -- precise "is this
//! actually unconditional" control-flow analysis is out of scope;
//! this is a nudge, not a certainty.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql803"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"LOOP") || end_immediately_before(ub, i).is_some() {
        i += 1;
        continue;
      }
      let Some(end_loop) = matching_end_loop(ub, i) else {
        i += 4;
        continue;
      };
      let loop_body = &upper[i + 4..end_loop];
      if let Some(rel) = find_raise_notice(loop_body) {
        let abs = i + 4 + rel;
        out.push(Diagnostic {
          code: "sql803",
          severity: Severity::Hint,
          message: "RAISE NOTICE inside a loop body -- a per-iteration notice can be noisy/slow on bulk operations".into(),
          range: crate::range_at(start + abs, start + abs + "RAISE".len()),
        });
      }
      i = end_loop + 3;
    }
  }
}

fn find_raise_notice(s: &str) -> Option<usize> {
  let ub = s.as_bytes();
  let mut i = 0usize;
  while i < ub.len() {
    if word_at(ub, i, b"RAISE") {
      let after = skip_ws(ub, i + 5);
      if word_at(ub, after, b"NOTICE") {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}

/// The matching `END LOOP` for the `LOOP` at `loop_pos`, tracking
/// nesting depth across every `LOOP`/`END LOOP` pair.
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
