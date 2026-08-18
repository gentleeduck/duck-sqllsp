//! sql763: `JSON_EXISTS(doc, 'literal')` where the literal path string
//! does not start with `$` -- not a valid SQL/JSON path expression.
//! PostgreSQL raises an error evaluating the path at runtime; the
//! parser accepts any string literal here since path validity isn't
//! checked until execution (verified empirically: no parse rejection).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql763"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSON_EXISTS") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSON_EXISTS".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSON_EXISTS".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      if let Some((lit_start, lit_end)) = first_string_literal(body, p, close) {
        let content = &body[lit_start + 1..lit_end];
        let trimmed = content.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('$') {
          out.push(Diagnostic {
            code: "sql763",
            severity: Severity::Error,
            message: "JSON_EXISTS path does not start with `$` -- not a valid SQL/JSON path expression".into(),
            range: crate::range_at(start + lit_start, start + lit_end + 1),
          });
        }
      }
      i = close + 1;
    }
  }
}

/// First single-quoted string literal's `(open_quote, close_quote)`
/// byte offsets strictly inside `(open+1, close)`.
fn first_string_literal(body: &str, open: usize, close: usize) -> Option<(usize, usize)> {
  let b = body.as_bytes();
  let mut i = open + 1;
  while i < close {
    if b[i] == b'\'' {
      let s = i;
      i += 1;
      while i < close && b[i] != b'\'' {
        i += 1;
      }
      if i < close {
        return Some((s, i));
      }
      return None;
    }
    i += 1;
  }
  None
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
