//! sql740: `NOT TRUE` / `NOT FALSE` -- negating a boolean literal is a
//! constant (`NOT TRUE` is FALSE, `NOT FALSE` is TRUE). In a predicate it
//! silently forces the branch on or off; almost always a debugging leftover.
//! `IS NOT TRUE` / `IS NOT FALSE` are not flagged -- their NULL handling is
//! meaningful. (Companion to sql686 not_not_double_negation.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql740"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      if ub[i] == b'\'' {
        i += 1;
        while i < n && ub[i] != b'\'' {
          i += 1;
        }
        i += 1;
        continue;
      }
      if !word_at(ub, i, b"NOT") {
        i += 1;
        continue;
      }
      // Skip `IS NOT TRUE/FALSE` (meaningful NULL handling).
      if preceding_word_is_is(ub, i) {
        i += 3;
        continue;
      }
      let w = skip_ws(ub, i + 3);
      let litlen = if word_at(ub, w, b"TRUE") {
        4
      } else if word_at(ub, w, b"FALSE") {
        5
      } else {
        0
      };
      if litlen > 0 {
        out.push(Diagnostic {
          code: "sql740",
          severity: Severity::Warning,
          message: "NOT of a boolean literal is a constant -- it forces the predicate on or off".into(),
          range: crate::range_at(start + i, start + w + litlen),
        });
        i = w + litlen;
        continue;
      }
      i += 3;
    }
  }
}

/// Whether the word immediately before position `i` is `IS`.
fn preceding_word_is_is(ub: &[u8], i: usize) -> bool {
  let mut e = i;
  while e > 0 && ub[e - 1].is_ascii_whitespace() {
    e -= 1;
  }
  let mut s = e;
  while s > 0 && is_word(ub[s - 1] as char) {
    s -= 1;
  }
  &ub[s..e] == b"IS"
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
