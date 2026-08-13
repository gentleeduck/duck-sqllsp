//! sql765: `JSON_QUERY(... WITH WRAPPER ... OMIT QUOTES)` -- OMIT
//! QUOTES is disallowed together with a wrapper (PostgreSQL raises an
//! error at query time; the parser accepts the combination -- verified
//! empirically). Only the plain `WITH WRAPPER` form is covered --
//! `WITH CONDITIONAL/UNCONDITIONAL [ARRAY] WRAPPER` variants are out
//! of scope to keep detection conservative.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql765"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSON_QUERY") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSON_QUERY".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSON_QUERY".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let call = &upper[p..=close];
      if call.contains("WITH WRAPPER") && call.contains("OMIT QUOTES") {
        out.push(Diagnostic {
          code: "sql765",
          severity: Severity::Error,
          message: "OMIT QUOTES cannot be used together with WITH WRAPPER in JSON_QUERY".into(),
          range: crate::range_at(start + p, start + close + 1),
        });
      }
      i = close + 1;
    }
  }
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
