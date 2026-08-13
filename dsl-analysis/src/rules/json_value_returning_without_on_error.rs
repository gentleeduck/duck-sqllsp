//! sql764: `JSON_VALUE(... RETURNING <type>)` narrowing the return
//! type with no `ON ERROR` clause. If the extracted value doesn't
//! convert to the target type, JSON_VALUE raises an unhandled runtime
//! error; an explicit `NULL ON ERROR` / `DEFAULT ... ON ERROR` avoids
//! that. Hint-level: valid SQL, just a missing safety net.

use crate::clause_scan::is_word;
use crate::textutil::find_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql764"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"JSON_VALUE") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + "JSON_VALUE".len());
      if ub.get(p) != Some(&b'(') {
        i += "JSON_VALUE".len();
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      let call = &upper[p..=close];
      if let Some(ret_rel) = find_word(call, "RETURNING")
        && !call.contains("ON ERROR")
      {
        let abs = p + ret_rel;
        out.push(Diagnostic {
          code: "sql764",
          severity: Severity::Hint,
          message: "JSON_VALUE narrows the return type but has no ON ERROR clause -- a conversion failure raises an unhandled runtime error".into(),
          range: crate::range_at(start + abs, start + abs + "RETURNING".len()),
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
