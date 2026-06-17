//! sql683: `CASE WHEN TRUE THEN ...` / `CASE WHEN FALSE THEN ...` -- the
//! first branch of a searched CASE tests a constant boolean. `WHEN TRUE` makes
//! the branch unconditional (the rest of the CASE is dead); `WHEN FALSE` makes
//! it unreachable. Either way it's a leftover debugging edit or a forgotten
//! real condition. (Only the leading `CASE WHEN <bool>` is flagged, so simple
//! `CASE x WHEN TRUE` value comparisons are left alone.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql683"
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
      if !word_at(ub, i, b"CASE") {
        i += 1;
        continue;
      }
      // `CASE WHEN <TRUE|FALSE> THEN` -- searched CASE, constant first cond.
      let w = skip_ws(ub, i + 4);
      if word_at(ub, w, b"WHEN") {
        let lit = skip_ws(ub, w + 4);
        let litlen = if word_at(ub, lit, b"TRUE") {
          4
        } else if word_at(ub, lit, b"FALSE") {
          5
        } else {
          0
        };
        if litlen > 0 {
          let t = skip_ws(ub, lit + litlen);
          if word_at(ub, t, b"THEN") {
            out.push(Diagnostic {
              code: "sql683",
              severity: Severity::Warning,
              message: "CASE WHEN with a constant TRUE/FALSE condition -- the branch is unconditional or dead".into(),
              range: crate::range_at(start + lit, start + lit + litlen),
            });
          }
        }
      }
      i += 4;
    }
  }
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
