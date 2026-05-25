//! sql203: `RAISE 'msg'` inside a PL/pgSQL body without a level
//! keyword (NOTICE/INFO/LOG/WARNING/EXCEPTION/DEBUG). PG defaults to
//! EXCEPTION which aborts the surrounding transaction -- almost never
//! the intended behaviour when the author wrote `RAISE 'debug %', x`.
//!
//! Heuristic: word-bounded RAISE followed directly by a string literal
//! (skipping whitespace) instead of a level keyword.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql203"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = body.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("RAISE") {
      let at = from + rel;
      if at > 0 {
        let prev = bytes[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' { from = at + 5; continue }
      }
      let after = at + "RAISE".len();
      if after >= bytes.len() { break }
      let next = bytes[after] as char;
      if !next.is_ascii_whitespace() { from = after; continue }
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1 }
      if k >= bytes.len() { break }
      if bytes[k] == b'\'' {
        // RAISE 'literal' -- missing level keyword.
        out.push(Diagnostic {
          code: "sql203",
          severity: Severity::Warning,
          message: "RAISE without level keyword -- defaults to EXCEPTION (aborts tx); use NOTICE/INFO/LOG/WARNING/DEBUG/EXCEPTION explicitly".into(),
          range: text_size::TextRange::new(((start + at) as u32).into(), ((start + at + 5) as u32).into()),
        });
      }
      from = k;
    }
  }
}
