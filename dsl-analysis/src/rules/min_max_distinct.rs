//! sql691: `min(DISTINCT x)` / `max(DISTINCT x)` -- DISTINCT has no effect on
//! MIN or MAX (the smallest / largest value is the same whether or not
//! duplicates are removed), so it's dead weight that only costs a sort/hash.
//! Drop the DISTINCT. (Companion to sql676 count_distinct_constant.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql691"
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
      if !word_at(ub, i, b"MIN") && !word_at(ub, i, b"MAX") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 3);
      if ub.get(p) != Some(&b'(') {
        i += 3;
        continue;
      }
      let d = skip_ws(ub, p + 1);
      if word_at(ub, d, b"DISTINCT") {
        out.push(Diagnostic {
          code: "sql691",
          severity: Severity::Hint,
          message: "DISTINCT has no effect on MIN/MAX -- drop it".into(),
          range: crate::range_at(start + d, start + d + 8),
        });
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
