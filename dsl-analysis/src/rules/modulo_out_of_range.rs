//! sql546: `WHERE x % 7 = 7` -- the result of `x % N` is always in the range
//! `(-N, N)`, so comparing it to a value whose magnitude is `>= N` can never
//! be true. `x % 2 = 2`, `id % 10 = 10`, etc. are dead predicates -- usually
//! an off-by-one (the author wanted `% N = 0` or a different divisor).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql546"
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
          // `% <N> = <M>` with N, M integer literals.
          let np = skip_ws(bytes, i + 1);
          if let Some((divisor, after_n)) = read_uint(bytes, np, n)
            && divisor > 0
          {
            let mut p = skip_ws(bytes, after_n);
            if bytes.get(p) == Some(&b'=') && bytes.get(p + 1) != Some(&b'=') {
              p = skip_ws(bytes, p + 1);
              if let Some((target, end)) = read_int(bytes, p, n)
                && target.abs() >= divisor
              {
                out.push(Diagnostic {
                  code: "sql546",
                  severity: Severity::Warning,
                  message: format!(
                    "`% {divisor} = {target}` never matches -- `x % {divisor}` is always between {} and {}",
                    -(divisor - 1),
                    divisor - 1
                  ),
                  range: crate::range_at(start + i, start + end),
                });
              }
            }
          }
        },
        _ => {},
      }
      i += 1;
    }
  }
}

fn read_uint(bytes: &[u8], start: usize, to: usize) -> Option<(i64, usize)> {
  let mut i = start;
  while i < to && bytes[i].is_ascii_digit() {
    i += 1;
  }
  if i == start {
    return None;
  }
  if matches!(bytes.get(i), Some(&b) if b == b'.' || b.is_ascii_alphabetic() || b == b'_') {
    return None;
  }
  let v: i64 = std::str::from_utf8(&bytes[start..i]).ok()?.parse().ok()?;
  Some((v, i))
}

fn read_int(bytes: &[u8], start: usize, to: usize) -> Option<(i64, usize)> {
  let mut i = start;
  if bytes.get(i) == Some(&b'-') {
    i += 1;
  }
  let (v, e) = read_uint(bytes, i, to)?;
  let signed = if start < i { -v } else { v };
  Some((signed, e))
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
