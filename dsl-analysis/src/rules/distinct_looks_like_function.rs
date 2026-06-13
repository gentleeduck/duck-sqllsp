//! sql539: `SELECT DISTINCT(col), other ...` (or `COUNT(DISTINCT(col))`) --
//! `DISTINCT` written as if it were a function. The parentheses are
//! misleading: `DISTINCT` is a keyword that deduplicates the *entire* row /
//! aggregate input, not just the parenthesised expression. The query may
//! still be correct, but readers (and the author) routinely misread it as
//! "distinct on this one column".

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql539"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i + 8 <= n {
      if ub[i..i + 8] != *b"DISTINCT" || (i > 0 && is_word(ub[i - 1] as char)) {
        i += 1;
        continue;
      }
      // The `(` must immediately follow DISTINCT (no space) -- that's the
      // function-call look-alike. `DISTINCT col` and `DISTINCT ON (...)`
      // both have a separating space and are fine.
      if ub.get(i + 8) == Some(&b'(') {
        let Some(close) = match_paren(ub, i + 8) else { break };
        out.push(Diagnostic {
          code: "sql539",
          severity: Severity::Hint,
          message: "`DISTINCT` is a keyword, not a function -- the parentheses are misleading; it deduplicates \
                    the whole row / aggregate input, not just this expression"
            .into(),
          range: crate::range_at(start + i, start + close + 1),
        });
        i = close + 1;
      } else {
        i += 8;
      }
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
