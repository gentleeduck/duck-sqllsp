//! sql562: `col int DEFAULT (SELECT max(id) FROM t)` -- a subquery in a column
//! DEFAULT. Postgres rejects it ("cannot use subquery in DEFAULT expression"):
//! a default is evaluated per-row without access to other rows/tables. Use a
//! trigger or compute the value in the INSERT instead.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql562"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 7 <= n {
      if &ub[i..i + 7] != b"DEFAULT" || (i > 0 && is_word(ub[i - 1] as char)) || is_word(*ub.get(i + 7).unwrap_or(&b' ') as char) {
        i += 1;
        continue;
      }
      let mut p = i + 7;
      while p < n && ub[p].is_ascii_whitespace() {
        p += 1;
      }
      if ub.get(p) != Some(&b'(') {
        i += 7;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // First non-whitespace token inside the parens is SELECT.
      let mut q = p + 1;
      while q < close && ub[q].is_ascii_whitespace() {
        q += 1;
      }
      if q + 6 <= close && &ub[q..q + 6] == b"SELECT" && !is_word(*ub.get(q + 6).unwrap_or(&b' ') as char) {
        out.push(Diagnostic {
          code: "sql562",
          severity: Severity::Error,
          message: "subquery in a column DEFAULT is not allowed -- use a trigger or set the value in the INSERT".into(),
          range: crate::range_at(start + i, start + close + 1),
        });
      }
      i = close + 1;
    }
  }
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
