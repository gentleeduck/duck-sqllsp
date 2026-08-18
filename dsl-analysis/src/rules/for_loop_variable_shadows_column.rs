//! sql799: a `FOR i IN ...` loop variable name shadows a column that
//! exists somewhere in the connected catalog -- classic PL/pgSQL
//! footgun (ambiguous column vs. variable reference inside the loop
//! body). Checked against the whole `Catalog` rather than a per-
//! statement `Scope`, since a PL/pgSQL function/DO body isn't resolved
//! against a FROM-clause scope the way a bare SELECT is.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql799"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if catalog.tables().next().is_none() {
      return;
    }
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while i < ub.len() {
      if !word_at(ub, i, b"FOR") {
        i += 1;
        continue;
      }
      let p = skip_ws(ub, i + 3);
      let name_start = p;
      let mut q = p;
      while q < ub.len() && is_word(ub[q] as char) {
        q += 1;
      }
      if q == name_start {
        i += 3;
        continue;
      }
      let after = skip_ws(ub, q);
      if !word_at(ub, after, b"IN") {
        i += 3;
        continue;
      }
      let name = &body[name_start..q];
      if !catalog.columns_named(name).is_empty() {
        out.push(Diagnostic {
          code: "sql799",
          severity: Severity::Hint,
          message: format!(
            "loop variable `{name}` shadows a column name of the same spelling -- ambiguous references inside the loop body"
          ),
          range: crate::range_at(start + name_start, start + q),
        });
      }
      i = after;
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
