//! sql675: `SELECT DISTINCT ... UNION SELECT ...` -- a branch begins with
//! DISTINCT but the set operation already deduplicates the combined result.
//! Plain UNION / INTERSECT / EXCEPT remove duplicate rows, so the per-branch
//! DISTINCT is wasted work (an extra sort/hash). Drop it -- or, if duplicates
//! across branches should survive, switch the set op to its ALL form.
//!
//! Conservative: only fires when every top-level set op deduplicates (no
//! `... ALL`), so the redundancy is unconditional. `DISTINCT ON (...)` is
//! left alone -- it selects specific rows and is not made redundant by UNION.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql675"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut has_dedup_setop = false;
    let mut has_all_setop = false;
    let mut distinct_positions: Vec<usize> = Vec::new();

    let mut depth = 0i32;
    let mut i = 0usize;
    while i < n {
      match ub[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        b'\'' => {
          i += 1;
          while i < n && ub[i] != b'\'' {
            i += 1;
          }
        },
        _ if depth == 0 => {
          if let Some(kw_len) = setop_at(ub, i) {
            let after = skip_ws(ub, i + kw_len);
            if word_at(ub, after, b"ALL") {
              has_all_setop = true;
            } else {
              has_dedup_setop = true;
            }
            i += kw_len;
            continue;
          }
          if word_at(ub, i, b"SELECT") {
            let d = skip_ws(ub, i + 6);
            if word_at(ub, d, b"DISTINCT") {
              let on = skip_ws(ub, d + 8);
              if !word_at(ub, on, b"ON") {
                distinct_positions.push(d);
              }
            }
          }
        },
        _ => {},
      }
      i += 1;
    }

    if has_dedup_setop && !has_all_setop {
      for d in distinct_positions {
        out.push(Diagnostic {
          code: "sql675",
          severity: Severity::Hint,
          message: "DISTINCT is redundant -- the enclosing UNION/INTERSECT/EXCEPT already deduplicates".into(),
          range: crate::range_at(start + d, start + d + 8),
        });
      }
    }
  }
}

/// Length of a set-operation keyword starting at `i` (word-bounded), or None.
fn setop_at(ub: &[u8], i: usize) -> Option<usize> {
  for kw in [&b"UNION"[..], &b"INTERSECT"[..], &b"EXCEPT"[..]] {
    if word_at(ub, i, kw) {
      return Some(kw.len());
    }
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
