//! sql725: `random() >= 1` / `random() < 0` -- `random()` returns a value in
//! the half-open interval [0, 1), so a comparison against a constant outside
//! that interval is always false. Usually a misremembered range (people often
//! assume [0, 1] or a percentage). Scale the result (`random() * 100`) or fix
//! the bound.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql725"
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
      if !word_at(ub, i, b"RANDOM") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 6);
      // Need empty parens: `random()`.
      if ub.get(p) != Some(&b'(') {
        i += 6;
        continue;
      }
      let q = skip_ws(ub, p + 1);
      if ub.get(q) != Some(&b')') {
        i = q;
        continue;
      }
      // Operator and constant after `)`.
      let mut r = skip_ws(ub, q + 1);
      let (op, oplen): (Op, usize) = match (ub.get(r), ub.get(r + 1)) {
        (Some(b'<'), Some(b'=')) => (Op::Le, 2),
        (Some(b'>'), Some(b'=')) => (Op::Ge, 2),
        (Some(b'<'), Some(b'>')) => (Op::Other, 2),
        (Some(b'<'), _) => (Op::Lt, 1),
        (Some(b'>'), _) => (Op::Gt, 1),
        (Some(b'='), _) => (Op::Eq, 1),
        _ => {
          i = q + 1;
          continue;
        },
      };
      r = skip_ws(ub, r + oplen);
      if let Some((s, e)) = number_token(ub, r, n)
        && let Ok(c) = upper[s..e].parse::<f64>()
        && op.always_false(c)
      {
        out.push(Diagnostic {
          code: "sql725",
          severity: Severity::Warning,
          message: "random() is in [0, 1) -- this comparison is always false".into(),
          range: crate::range_at(start + i, start + e),
        });
        i = e;
        continue;
      }
      i = q + 1;
    }
  }
}

enum Op {
  Lt,
  Le,
  Gt,
  Ge,
  Eq,
  Other,
}

impl Op {
  fn always_false(&self, c: f64) -> bool {
    match self {
      Op::Lt => c <= 0.0,
      Op::Le => c < 0.0,
      Op::Gt => c >= 1.0,
      Op::Ge => c >= 1.0,
      Op::Eq => !(0.0..1.0).contains(&c),
      Op::Other => false,
    }
  }
}

/// A numeric literal starting at `i` (optional sign, digits, dot).
fn number_token(ub: &[u8], i: usize, n: usize) -> Option<(usize, usize)> {
  let mut e = i;
  if e < n && (ub[e] == b'-' || ub[e] == b'+') {
    e += 1;
  }
  let digits_start = e;
  while e < n && (ub[e].is_ascii_digit() || ub[e] == b'.') {
    e += 1;
  }
  (e > digits_start).then_some((i, e))
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
