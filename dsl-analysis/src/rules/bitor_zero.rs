//! sql728: `x | 0` / `0 | x` -- a bitwise OR with 0 leaves the value
//! unchanged, so the operand is dead. Almost always a typo for a real mask or
//! a disabled flag. (Companion to sql713 bitand_zero and sql714 bitwise_self.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql728"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let ub = body.as_bytes();
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
        // Single `|` (bitwise OR), not `||` (concat).
        b'|' if ub.get(i + 1) != Some(&b'|') && (i == 0 || ub[i - 1] != b'|') => {
          let Some(l) = last_non_ws(ub, i) else {
            i += 1;
            continue;
          };
          if !is_operand_end(ub[l]) {
            i += 1;
            continue;
          }
          let r = skip_ws(ub, i + 1);
          if is_zero(ub, r) {
            out.push(diag(start + l, start + r + 1));
          } else if ub[l] == b'0' && (l == 0 || !is_word(ub[l - 1] as char) && ub[l - 1] != b'.') {
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
    code: "sql728",
    severity: Severity::Hint,
    message: "bitwise OR with 0 is a no-op -- it leaves the value unchanged".into(),
    range: crate::range_at(s, e),
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
