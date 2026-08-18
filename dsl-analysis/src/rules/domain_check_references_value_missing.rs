//! sql777: `CREATE DOMAIN ... CHECK (expr)` where `expr` never
//! references `VALUE` -- evaluates to the same result for every input,
//! so the constraint either always fires or never does regardless of
//! what's being validated.

use crate::clause_scan::is_word;
use crate::textutil::contains_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql777"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE DOMAIN") {
      return;
    }
    let ub = upper.as_bytes();
    let Some(chk) = find_word_from(ub, 0, b"CHECK") else { return };
    let p = skip_ws(ub, chk + "CHECK".len());
    if ub.get(p) != Some(&b'(') {
      return;
    }
    let Some(close) = match_paren(ub, p) else { return };
    let inner = &upper[p + 1..close];
    if !contains_word(inner, "VALUE") {
      out.push(Diagnostic {
        code: "sql777",
        severity: Severity::Warning,
        message: "domain CHECK does not reference VALUE -- evaluates to the same result for every input".into(),
        range: crate::range_at(start + p, start + close + 1),
      });
    }
  }
}

fn find_word_from(ub: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= ub.len() {
    if &ub[i..i + w.len()] == w
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
    {
      return Some(i);
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

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
