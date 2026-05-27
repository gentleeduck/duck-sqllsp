//! sql329: `substring(text FROM <number>)` without a matching `FOR`.
//!
//! PG returns the rest of the string from the start position when FOR
//! is omitted, which is rarely what the author wanted -- almost every
//! sighting in code review turns out to be a typo for `FOR n`. Make
//! it explicit.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql329"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let mut i = 0;
    while i + 10 <= bytes.len() {
      if &upper[i..i + 9] == "SUBSTRING" && (i == 0 || !is_word(bytes[i - 1] as char)) {
        let mut j = i + 9;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
          j += 1
        }
        if j < bytes.len() && bytes[j] == b'(' {
          let inner_start = j + 1;
          let Some(close) = match_paren(bytes, j) else { break };
          let inner = &upper[inner_start..close];
          // Must contain ` FROM ` and must NOT contain ` FOR `.
          if inner.contains(" FROM ") && !inner.contains(" FOR ") {
            let abs_s = start + i;
            let abs_e = start + close + 1;
            out.push(Diagnostic {
              code: "sql329",
              severity: Severity::Hint,
              message:
                "substring(... FROM n) without FOR returns the rest of the string -- add FOR <len> to be explicit"
                  .into(),
              range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
          }
          i = close + 1;
          continue;
        }
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
