//! sql697: `degrees(radians(x))` / `radians(degrees(x))` -- converting an
//! angle to the other unit and immediately back is the identity, so both
//! calls are dead weight (modulo floating-point rounding). Drop them and use
//! `x` directly. (Companion to sql551 redundant_nested_function.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql697"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      let inner: &[u8] = if word_at(ub, i, b"DEGREES") {
        b"RADIANS"
      } else if word_at(ub, i, b"RADIANS") {
        b"DEGREES"
      } else {
        i += 1;
        continue;
      };
      let p = skip_ws(ub, i + 7);
      if ub.get(p) != Some(&b'(') {
        i += 7;
        continue;
      }
      let q = skip_ws(ub, p + 1);
      if word_at(ub, q, inner) && ub.get(skip_ws(ub, q + inner.len())) == Some(&b'(') {
        out.push(Diagnostic {
          code: "sql697",
          severity: Severity::Hint,
          message: "degrees()/radians() round-trip is the identity -- the conversions cancel out".into(),
          range: crate::range_at(start + i, start + q + inner.len()),
        });
      }
      i += 7;
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
