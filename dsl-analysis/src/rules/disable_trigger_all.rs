//! sql576: `ALTER TABLE t DISABLE TRIGGER ALL` -- disables *every* trigger on
//! the table, including the internal RI triggers that enforce foreign keys.
//! Bulk loads done this way can leave dangling references and skip audit /
//! business-logic triggers. Prefer `DISABLE TRIGGER USER` (keeps FK checks),
//! or re-validate constraints afterwards. Easy to leave on by accident.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql576"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if let Some(at) = find_phrase(upper.as_bytes(), &["DISABLE", "TRIGGER", "ALL"]) {
      out.push(Diagnostic {
        code: "sql576",
        severity: Severity::Warning,
        message: "DISABLE TRIGGER ALL also disables foreign-key enforcement -- use DISABLE TRIGGER USER to keep FK checks".into(),
        range: crate::range_at(start + at, start + at + 7),
      });
    }
  }
}

/// Offset of the first word of `words` when they appear consecutively
/// (whitespace-separated, word-bounded) in `ub`.
fn find_phrase(ub: &[u8], words: &[&str]) -> Option<usize> {
  let n = ub.len();
  let first = words[0].as_bytes();
  let mut i = 0usize;
  while i + first.len() <= n {
    if &ub[i..i + first.len()] == first
      && (i == 0 || !is_word(ub[i - 1]))
      && matches_rest(ub, i + first.len(), &words[1..])
    {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn matches_rest(ub: &[u8], mut p: usize, words: &[&str]) -> bool {
  let n = ub.len();
  for w in words {
    while p < n && ub[p].is_ascii_whitespace() {
      p += 1;
    }
    let wb = w.as_bytes();
    if p + wb.len() <= n && &ub[p..p + wb.len()] == wb && (p + wb.len() == n || !is_word(ub[p + wb.len()])) {
      p += wb.len();
    } else {
      return false;
    }
  }
  true
}

fn is_word(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}
