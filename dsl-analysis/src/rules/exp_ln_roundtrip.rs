//! sql727: `exp(ln(x))` / `ln(exp(x))` -- exp and ln are inverses, so a
//! round-trip is the identity (modulo float rounding and the domain x > 0).
//! Both calls are dead weight; use `x` directly. (Companion to sql697
//! degrees_radians_roundtrip and sql551 redundant_nested_function.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql727"
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
      let (inner, olen): (&[u8], usize) = if word_at(ub, i, b"EXP") {
        (b"LN", 3)
      } else if word_at(ub, i, b"LN") {
        (b"EXP", 2)
      } else {
        i += 1;
        continue;
      };
      let p = skip_ws(ub, i + olen);
      if ub.get(p) != Some(&b'(') {
        i += olen;
        continue;
      }
      let q = skip_ws(ub, p + 1);
      if word_at(ub, q, inner) && ub.get(skip_ws(ub, q + inner.len())) == Some(&b'(') {
        out.push(Diagnostic {
          code: "sql727",
          severity: Severity::Hint,
          message: "exp()/ln() round-trip is the identity -- the inverse calls cancel out".into(),
          range: crate::range_at(start + i, start + q + inner.len()),
        });
      }
      i += olen;
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
