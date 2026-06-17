//! sql689: `col % col` -- a column modulo itself is always 0 (or a
//! division-by-zero error when the column is 0, or NULL). The result is a
//! constant, almost always a typo for a different right-hand operand.
//! (Companion to sql565 self_arithmetic for `col - col` / `col / col` and
//! sql677 modulo_by_one.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql689"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
      match bytes[i] {
        b'\'' => {
          i += 1;
          while i < n && bytes[i] != b'\'' {
            i += 1;
          }
        },
        b'%' => {
          if let (Some((ls, le)), Some((rs, re))) = (ident_before(body, i), ident_after(body, i + 1))
            && body[ls..le].eq_ignore_ascii_case(&body[rs..re])
          {
            let col = &body[ls..le];
            out.push(Diagnostic {
              code: "sql689",
              severity: Severity::Warning,
              message: format!("`{col} % {col}` is always 0 (or NULL) -- did you mean a different operand?"),
              range: crate::range_at(start + ls, start + re),
            });
            i = re;
            continue;
          }
        },
        _ => {},
      }
      i += 1;
    }
  }
}

fn ident_before(body: &str, at: usize) -> Option<(usize, usize)> {
  let bytes = body.as_bytes();
  let mut end = at;
  while end > 0 && bytes[end - 1].is_ascii_whitespace() {
    end -= 1;
  }
  let mut s = end;
  while s > 0 && (is_word(bytes[s - 1] as char) || bytes[s - 1] == b'.' || bytes[s - 1] == b'"') {
    s -= 1;
  }
  // Must be a non-numeric identifier (so `5 % 5` isn't flagged).
  if s == end || bytes[s].is_ascii_digit() {
    return None;
  }
  Some((s, end))
}

fn ident_after(body: &str, at: usize) -> Option<(usize, usize)> {
  let bytes = body.as_bytes();
  let n = bytes.len();
  let mut s = at;
  while s < n && bytes[s].is_ascii_whitespace() {
    s += 1;
  }
  let mut e = s;
  while e < n && (is_word(bytes[e] as char) || bytes[e] == b'.' || bytes[e] == b'"') {
    e += 1;
  }
  if e == s || bytes[s].is_ascii_digit() {
    return None;
  }
  Some((s, e))
}
