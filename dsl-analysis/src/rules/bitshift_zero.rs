//! sql729: `x << 0` / `x >> 0` -- shifting by 0 bits leaves the value
//! unchanged, so the shift is a no-op. Usually a typo for a real shift amount.
//! (Companion to sql728 bitor_zero.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql729"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let ub = body.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 1 < n {
      match ub[i] {
        b'\'' => {
          i += 1;
          while i < n && ub[i] != b'\'' {
            i += 1;
          }
        },
        // `<<` or `>>` with a literal 0 right operand.
        b'<' | b'>' if ub[i + 1] == ub[i] => {
          let Some(l) = last_non_ws(ub, i) else {
            i += 2;
            continue;
          };
          let r = skip_ws(ub, i + 2);
          if is_operand_end(ub[l]) && is_zero(ub, r) {
            let opc = ub[i] as char;
            out.push(Diagnostic {
              code: "sql729",
              severity: Severity::Hint,
              message: format!("shift by 0 (`{opc}{opc} 0`) is a no-op -- it leaves the value unchanged"),
              range: crate::range_at(start + i, start + r + 1),
            });
          }
          i += 2;
          continue;
        },
        _ => {},
      }
      i += 1;
    }
  }
}

fn is_zero(ub: &[u8], i: usize) -> bool {
  ub.get(i) == Some(&b'0') && !matches!(ub.get(i + 1), Some(c) if c.is_ascii_digit() || *c == b'.' || is_word(*c as char))
}

fn is_operand_end(c: u8) -> bool {
  c.is_ascii_alphanumeric() || c == b'_' || c == b')' || c == b']' || c == b'"' || c == b'\''
}

fn last_non_ws(ub: &[u8], i: usize) -> Option<usize> {
  let mut j = i;
  while j > 0 {
    j -= 1;
    if !ub[j].is_ascii_whitespace() {
      return Some(j);
    }
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
