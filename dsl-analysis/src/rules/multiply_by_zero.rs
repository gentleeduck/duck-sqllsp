//! sql681: `x * 0` / `0 * x` -- multiplying by the literal 0 is always 0
//! (NULL when `x` is NULL), so the expression is a constant. Almost always a
//! typo (a different factor was meant) or a disabled term left in by mistake.
//! (Companion to sql489 where_arith_identity and sql565 self_arithmetic.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql681"
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
      match ub[i] {
        b'\'' => {
          i += 1;
          while i < n && ub[i] != b'\'' {
            i += 1;
          }
        },
        b'*' => {
          // Distinguish multiplication from a `*` wildcard: a multiply has an
          // operand-ending char immediately to its left.
          let Some(l) = last_non_ws(ub, i) else {
            i += 1;
            continue;
          };
          if !is_operand_end(ub[l]) {
            i += 1;
            continue;
          }
          // Right operand is a bare 0?
          let r = skip_ws(ub, i + 1);
          if is_zero(ub, r) {
            out.push(diag(start + i, start + r + 1));
          } else if ub[l] == b'0' && (l == 0 || !is_word(ub[l - 1] as char) && ub[l - 1] != b'.') {
            // Left operand is a bare 0.
            out.push(diag(start + l, start + i + 1));
          }
        },
        _ => {},
      }
      i += 1;
    }
  }
}

fn diag(s: usize, e: usize) -> Diagnostic {
  Diagnostic {
    code: "sql681",
    severity: Severity::Warning,
    message: "multiplication by 0 is always 0 -- check the factor".into(),
    range: crate::range_at(s, e),
  }
}

/// `0` as a complete integer literal at `i` (not `0.5`, `00`, `0x`...).
fn is_zero(ub: &[u8], i: usize) -> bool {
  ub.get(i) == Some(&b'0') && !matches!(ub.get(i + 1), Some(c) if c.is_ascii_digit() || *c == b'.' || is_word(*c as char))
}

/// A char that can end an operand (so a following `*` is multiplication).
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
