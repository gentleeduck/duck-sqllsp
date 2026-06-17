//! sql682: `COALESCE(COUNT(...), 0)` -- COUNT never returns NULL. A grouped
//! or scalar `count(...)` returns 0 for an empty input, not NULL, so wrapping
//! it in COALESCE (almost always with a 0 fallback) is dead weight. Drop the
//! COALESCE and use the COUNT directly. (Companion to sql493 coalesce_not_null
//! for NOT NULL columns.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql682"
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
      if !word_at(ub, i, b"COALESCE") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 8);
      if ub.get(p) != Some(&b'(') {
        i += 8;
        continue;
      }
      // First argument starts with `COUNT(`.
      let a = skip_ws(ub, p + 1);
      if word_at(ub, a, b"COUNT") {
        let c = skip_ws(ub, a + 5);
        if ub.get(c) == Some(&b'(') {
          out.push(Diagnostic {
            code: "sql682",
            severity: Severity::Hint,
            message: "COUNT never returns NULL -- the COALESCE wrapper is redundant".into(),
            range: crate::range_at(start + i, start + a + 5),
          });
        }
      }
      i = p + 1;
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
