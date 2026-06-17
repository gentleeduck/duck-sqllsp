//! sql686: `NOT NOT x` / `NOT (NOT x)` -- a double negation cancels out. The
//! two NOTs leave the predicate unchanged (`NOT NOT x` is just `x IS TRUE`),
//! so they're dead weight, usually a leftover from editing a condition. Drop
//! both. (Companion to sql088 not_is_null and the not_paren_* checks.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql686"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      if ub[i] == b'\'' {
        i += 1;
        while i < n && ub[i] != b'\'' {
          i += 1;
        }
        i += 1;
        continue;
      }
      if !word_at(ub, i, b"NOT") {
        i += 1;
        continue;
      }
      // NOT [ ( ] NOT
      let mut p = skip_ws(ub, i + 3);
      if ub.get(p) == Some(&b'(') {
        p = skip_ws(ub, p + 1);
      }
      if word_at(ub, p, b"NOT") {
        out.push(Diagnostic {
          code: "sql686",
          severity: Severity::Hint,
          message: "double negation (NOT NOT) cancels out -- drop both NOTs".into(),
          range: crate::range_at(start + i, start + p + 3),
        });
        i = p + 3;
        continue;
      }
      i += 3;
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
