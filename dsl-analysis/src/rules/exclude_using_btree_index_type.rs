//! sql775: `EXCLUDE USING btree/hash/brin/gin (...)` -- these access
//! methods do not support exclusion constraints in PostgreSQL (only
//! gist and spgist do). PG rejects the constraint at DDL time.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const UNSUPPORTED: &[&str] = &["BTREE", "HASH", "BRIN", "GIN"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql775"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let mut i = 0usize;
    while let Some(excl) = find_word_from(ub, i, b"EXCLUDE") {
      let mut j = skip_ws(ub, excl + "EXCLUDE".len());
      if !word_at(ub, j, b"USING") {
        i = excl + "EXCLUDE".len();
        continue;
      }
      j = skip_ws(ub, j + "USING".len());
      let am_start = j;
      while j < ub.len() && is_word(ub[j] as char) {
        j += 1;
      }
      let am = &upper[am_start..j];
      if let Some(bad) = UNSUPPORTED.iter().find(|a| am.eq_ignore_ascii_case(a)) {
        out.push(Diagnostic {
          code: "sql775",
          severity: Severity::Error,
          message: format!("EXCLUDE USING {bad} -- exclusion constraints require gist or spgist, not {bad}"),
          range: crate::range_at(start + am_start, start + j),
        });
      }
      i = j;
    }
  }
}

fn find_word_from(ub: &[u8], from: usize, w: &[u8]) -> Option<usize> {
  let mut i = from;
  while i + w.len() <= ub.len() {
    if &ub[i..i + w.len()] == w
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + w.len() == ub.len() || !is_word(ub[i + w.len()] as char))
    {
      return Some(i);
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
