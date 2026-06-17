//! sql734: `x ILIKE 'plain'` -- an ILIKE pattern with no `%` or `_` wildcard
//! is just a case-insensitive equality test. `lower(x) = lower('plain')` (with
//! a matching functional index) or a citext column is clearer and can use an
//! index. (Mirror of sql052 like_without_wildcard for the ILIKE operator.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql734"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let bb = body.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      if !word_at(ub, i, b"ILIKE") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 5);
      if bb.get(p) == Some(&b'\'') {
        let mut j = p + 1;
        while j < n && bb[j] != b'\'' {
          j += 1;
        }
        let pat = &body[p + 1..j.min(n)];
        if j < n && !pat.contains('%') && !pat.contains('_') {
          out.push(Diagnostic {
            code: "sql734",
            severity: Severity::Hint,
            message: "ILIKE with no wildcard is just case-insensitive equality -- use lower(x) = ... or citext".into(),
            range: crate::range_at(start + i, start + (j + 1).min(n)),
          });
        }
        i = (j + 1).min(n);
        continue;
      }
      i += 5;
    }
  }
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
