//! sql533: `col BETWEEN 5 AND 5` -- the lower and upper bounds are the same
//! value, so the range degenerates to `col = 5`. `col NOT BETWEEN 5 AND 5`
//! is `col <> 5`. Writing it as a range obscures the intent and is usually a
//! placeholder left half-edited. Only simple identical literal/identifier
//! bounds are flagged.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql533"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let bytes = body.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 7 <= n {
      if ub[i..i + 7] != *b"BETWEEN" || (i > 0 && is_word(ub[i - 1] as char)) || is_word(*ub.get(i + 7).unwrap_or(&b' ') as char) {
        i += 1;
        continue;
      }
      // Low operand.
      let Some((ls, le)) = read_operand(bytes, i + 7) else {
        i += 7;
        continue;
      };
      // AND keyword.
      let p = skip_ws(bytes, le);
      if !(p + 3 <= n && ub[p..p + 3] == *b"AND" && !is_word(*ub.get(p + 3).unwrap_or(&b' ') as char)) {
        i = le;
        continue;
      }
      // High operand.
      let Some((hs, he)) = read_operand(bytes, p + 3) else {
        i = p + 3;
        continue;
      };
      // The high bound must be complete: next token can't continue the
      // expression with an operator (e.g. `5 AND 5 + 1`).
      let after = skip_ws(bytes, he);
      let complete = match bytes.get(after) {
        None => true,
        Some(&b) => b == b')' || b == b';' || b == b',' || b.is_ascii_alphabetic(),
      };
      if complete && operands_eq(&body[ls..le], &body[hs..he]) {
        let is_not = preceded_by_not(ub, i);
        let op = if is_not { "<>" } else { "=" };
        let kw = if is_not { "NOT BETWEEN" } else { "BETWEEN" };
        let span_start = if is_not { not_start(ub, i) } else { i };
        out.push(Diagnostic {
          code: "sql533",
          severity: Severity::Warning,
          message: format!("`{kw} {0} AND {0}` -- bounds are equal; use `{op} {0}`", body[ls..le].trim()),
          range: crate::range_at(start + span_start, start + he),
        });
      }
      i = he;
    }
  }
}

fn operands_eq(a: &str, b: &str) -> bool {
  let (a, b) = (a.trim(), b.trim());
  if a.contains('\'') || b.contains('\'') {
    a == b
  } else {
    a.eq_ignore_ascii_case(b)
  }
}

/// Read a single simple operand (number, single-quoted string, or
/// [qualified] identifier) starting at/after `pos`. Returns its (start, end).
fn read_operand(bytes: &[u8], pos: usize) -> Option<(usize, usize)> {
  let s = skip_ws(bytes, pos);
  let first = *bytes.get(s)?;
  if first == b'\'' {
    let mut i = s + 1;
    while i < bytes.len() && bytes[i] != b'\'' {
      i += 1;
    }
    if i >= bytes.len() {
      return None;
    }
    return Some((s, i + 1));
  }
  let numeric = first == b'-' || first == b'.' || first.is_ascii_digit();
  let identish = first.is_ascii_alphabetic() || first == b'_' || first == b'"';
  if !numeric && !identish {
    return None;
  }
  let mut i = s + 1;
  while i < bytes.len() {
    let b = bytes[i];
    let ok = if numeric { b.is_ascii_digit() || b == b'.' } else { is_word(b as char) || b == b'.' || b == b'"' };
    if ok {
      i += 1;
    } else {
      break;
    }
  }
  Some((s, i))
}

fn preceded_by_not(ub: &[u8], at: usize) -> bool {
  not_word_before(ub, at).is_some()
}

fn not_start(ub: &[u8], at: usize) -> usize {
  not_word_before(ub, at).unwrap_or(at)
}

fn not_word_before(ub: &[u8], at: usize) -> Option<usize> {
  let mut j = at;
  while j > 0 && ub[j - 1].is_ascii_whitespace() {
    j -= 1;
  }
  let end = j;
  while j > 0 && is_word(ub[j - 1] as char) {
    j -= 1;
  }
  if end - j == 3 && ub[j..end].eq_ignore_ascii_case(b"NOT") { Some(j) } else { None }
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
