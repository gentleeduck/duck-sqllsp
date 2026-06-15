//! sql570: `EXISTS (SELECT DISTINCT ...)` -- the DISTINCT is dead weight.
//! EXISTS only checks whether at least one row exists; deduplicating the rows
//! first can't change that (and costs a sort/hash). Drop the DISTINCT.
//! (Companion to sql525 / sql569 for LIMIT / ORDER BY in EXISTS.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql570"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 6 <= n {
      if ub[i..i + 6] != *b"EXISTS" || (i > 0 && is_word(ub[i - 1] as char)) || (i + 6 < n && is_word(ub[i + 6] as char)) {
        i += 1;
        continue;
      }
      let mut p = i + 6;
      while p < n && ub[p].is_ascii_whitespace() {
        p += 1;
      }
      if ub.get(p) != Some(&b'(') {
        i += 6;
        continue;
      }
      let Some(close) = match_paren(ub, p) else { break };
      // `( SELECT DISTINCT ...` -- DISTINCT immediately after the SELECT.
      let mut q = skip_ws(ub, p + 1);
      if q + 6 <= close && &ub[q..q + 6] == b"SELECT" && !is_word(ub[q + 6] as char) {
        q = skip_ws(ub, q + 6);
        if q + 8 <= close && &ub[q..q + 8] == b"DISTINCT" && !is_word(ub[q + 8] as char) {
          out.push(Diagnostic {
            code: "sql570",
            severity: Severity::Hint,
            message: "DISTINCT inside EXISTS is pointless -- EXISTS only checks for any row".into(),
            range: crate::range_at(start + q, start + q + 8),
          });
        }
      }
      i = close + 1;
    }
  }
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
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
