//! sql673: `x BETWEEN NULL AND y` / `x BETWEEN y AND NULL` -- a NULL bound
//! makes the whole range test evaluate to NULL (never TRUE), so the row can
//! never match. Almost always a missing value or a typo; supply a real bound
//! or rewrite the predicate. (Companion to sql011 between_self_bound and the
//! between_reversed / between_equal_bounds checks.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql673"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 7 <= n {
      if &ub[i..i + 7] != b"BETWEEN" || (i > 0 && is_word(ub[i - 1] as char)) || (i + 7 < n && is_word(ub[i + 7] as char)) {
        i += 1;
        continue;
      }
      let mut p = skip_ws(ub, i + 7);
      // BETWEEN [A]SYMMETRIC <lo> AND <hi>
      for kw in [&b"SYMMETRIC"[..], &b"ASYMMETRIC"[..]] {
        if word_at(ub, p, kw) {
          p = skip_ws(ub, p + kw.len());
          break;
        }
      }
      // Low bound is a bare NULL.
      if word_at(ub, p, b"NULL") {
        out.push(diag(start + p, start + p + 4));
      }
      // High bound: the token right after the separating top-level AND.
      if let Some(and) = top_level_and(ub, p, n) {
        let h = skip_ws(ub, and + 3);
        if word_at(ub, h, b"NULL") {
          out.push(diag(start + h, start + h + 4));
        }
      }
      i += 7;
    }
  }
}

fn diag(s: usize, e: usize) -> Diagnostic {
  Diagnostic {
    code: "sql673",
    severity: Severity::Warning,
    message: "NULL bound in BETWEEN -- the range test is always NULL, so the row never matches".into(),
    range: crate::range_at(s, e),
  }
}

/// First word-bounded top-level `AND` at or after `from` (skips parens and
/// string literals); this is the separator between BETWEEN's two bounds.
fn top_level_and(ub: &[u8], from: usize, to: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = from;
  while i < to {
    match ub[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < to && ub[i] != b'\'' {
          i += 1;
        }
      },
      b'A' if depth == 0 && word_at(ub, i, b"AND") => return Some(i),
      _ => {},
    }
    i += 1;
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
