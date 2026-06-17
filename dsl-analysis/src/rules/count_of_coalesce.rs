//! sql696: `count(coalesce(x, 0))` -- COUNT only skips NULLs, and COALESCE
//! with a non-NULL fallback never produces one, so this counts every row,
//! exactly like `count(*)`. The COALESCE defeats the point of `count(x)`
//! (counting only non-NULL `x`). Either drop the COALESCE to count non-NULLs,
//! or use `count(*)` to count rows. (DISTINCT is left alone -- there COALESCE
//! changes the distinct set. Companion to sql682 coalesce_count_redundant.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql696"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();

    let mut i = 0usize;
    while i < n {
      if !word_at(ub, i, b"COUNT") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 5);
      if ub.get(p) != Some(&b'(') {
        i += 5;
        continue;
      }
      // count( COALESCE( ... ) ) -- but not count(DISTINCT coalesce(...)).
      let a = skip_ws(ub, p + 1);
      if word_at(ub, a, b"COALESCE") {
        let c = skip_ws(ub, a + 8);
        if ub.get(c) == Some(&b'(') {
          out.push(Diagnostic {
            code: "sql696",
            severity: Severity::Warning,
            message: "count(coalesce(...)) counts every row like count(*) -- the COALESCE defeats count(x)".into(),
            range: crate::range_at(start + a, start + a + 8),
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
