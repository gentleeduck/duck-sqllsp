//! sql781: `jsonb_array_length('{"a":1}'::jsonb)` -- the argument is a
//! jsonb literal whose content is an object, not an array. PostgreSQL
//! raises 22023 ("cannot get array length of a non-array") at runtime;
//! the object-ness is knowable statically here because it's a literal.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql781"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSONB_ARRAY_LENGTH") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSONB_ARRAY_LENGTH".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSONB_ARRAY_LENGTH".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      if let Some((lit_s, lit_e)) = jsonb_literal_cast(body, &upper, p, close) {
        let content = body[lit_s + 1..lit_e].trim();
        if content.starts_with('{') {
          out.push(Diagnostic {
            code: "sql781",
            severity: Severity::Error,
            message: "jsonb_array_length on an object literal -- raises 22023 at runtime".into(),
            range: crate::range_at(start + lit_s, start + lit_e + 1),
          });
        }
      }
      i = close + 1;
    }
  }
}

/// A `'...'::jsonb` or `'...'::json` cast strictly inside `(open, close)`.
/// Returns the string literal's `(open_quote, close_quote)` offsets.
fn jsonb_literal_cast(body: &str, upper: &str, open: usize, close: usize) -> Option<(usize, usize)> {
  let b = body.as_bytes();
  let mut i = open + 1;
  while i < close {
    if b[i] == b'\'' {
      let s = i;
      i += 1;
      while i < close && b[i] != b'\'' {
        i += 1;
      }
      if i >= close {
        return None;
      }
      let e = i;
      let after = upper[i + 1..close].trim_start();
      if let Some(rest) = after.strip_prefix("::JSONB").or_else(|| after.strip_prefix("::JSON")) {
        let _ = rest;
        return Some((s, e));
      }
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
