//! sql756: `string_agg(x)` / `jsonb_object_agg(k)` -- a required second
//! argument is missing. string_agg needs a delimiter (`string_agg(x, ',')`)
//! and jsonb_object_agg needs both key and value. The one-argument forms do
//! not exist, so PostgreSQL raises 42883 ("function ... does not exist").
//! (array_agg / json_agg legitimately take a single argument and are not
//! flagged.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql756"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      let (name, len): (&str, usize) = if word_at(ub, i, b"STRING_AGG") {
        ("string_agg", 10)
      } else if word_at(ub, i, b"JSONB_OBJECT_AGG") {
        ("jsonb_object_agg", 16)
      } else {
        i += 1;
        continue;
      };
      let p = skip_ws(ub, i + len);
      if ub.get(p) != Some(&b'(') {
        i += len;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // Exactly one argument (no top-level comma) -> missing required arg.
      if top_level_comma(ub, p + 1, close).is_none() && skip_ws(ub, p + 1) != close {
        out.push(Diagnostic {
          code: "sql756",
          severity: Severity::Error,
          message: format!("{name}() is missing its required second argument -- raises 42883 at runtime"),
          range: crate::range_at(start + i, start + close + 1),
        });
      }
      i = close + 1;
    }
  }
}

fn top_level_comma(ub: &[u8], from: usize, to: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = from;
  while i < to {
    match ub[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < to && ub[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => return Some(i),
      _ => {},
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
          i += 1
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
