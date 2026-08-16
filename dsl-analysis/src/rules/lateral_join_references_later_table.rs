//! sql788: a `LATERAL (...)` subquery references a table alias that's
//! introduced later in the same FROM/JOIN list -- LATERAL can only see
//! items to its left, so this is out of scope (PG raises "missing
//! FROM-clause entry"). Uses `Scope`'s already-resolved binding
//! positions rather than hand-parsing FROM/JOIN order.

use crate::clause_scan::is_word;
use crate::textutil::contains_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql788"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"LATERAL") {
        i += 1;
        continue;
      }
      let lat_pos = i;
      let p = skip_ws(ub, i + "LATERAL".len());
      let open = if ub.get(p) == Some(&b'(') {
        Some(p)
      } else {
        // LATERAL fn(args) -- find the `(` after the function name.
        let mut q = p;
        while q < ub.len() && is_word(ub[q] as char) {
          q += 1;
        }
        let q2 = skip_ws(ub, q);
        (ub.get(q2) == Some(&b'(')).then_some(q2)
      };
      let Some(open) = open else {
        i += "LATERAL".len();
        continue;
      };
      let Some(close) = match_paren(ub, open) else { break };
      let lat_body_upper = &upper[open + 1..close];
      let lat_abs = start + lat_pos;
      for b in scope.tables() {
        let ref_start: usize = u32::from(b.table.range.start()) as usize;
        if ref_start <= lat_abs {
          continue;
        }
        if contains_word(lat_body_upper, &b.alias.to_ascii_uppercase()) {
          out.push(Diagnostic {
            code: "sql788",
            severity: Severity::Error,
            message: format!(
              "LATERAL references `{}`, which is introduced later in the FROM/JOIN list -- out of scope here",
              b.alias
            ),
            range: crate::range_at(start + open, start + close + 1),
          });
          break;
        }
      }
      i = close + 1;
    }
  }
}

fn word_at(ub: &[u8], i: usize, w: &[u8]) -> bool {
  i + w.len() <= ub.len()
    && &ub[i..i + w.len()] == w
    && (i == 0 || !is_word(ub[i - 1] as char))
    && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
