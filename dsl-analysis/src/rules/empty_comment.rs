//! sql091: `COMMENT ON ... IS ''` -- empty comment string. PG accepts
//! it but it usually means the author forgot to write the doc. Hint.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql091"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    // Don't strip noise here: the rule's job is to inspect the
    // string literal AFTER `IS`, and our stripper turns `''` into
    // spaces (which then look like a missing-empty-string).
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.starts_with("COMMENT ") {
      return;
    }
    if !upper.contains(" IS ") {
      return;
    }
    // Find the `IS` keyword, then the following single-quoted string.
    let bytes = body.as_bytes();
    let Some(is_idx) = upper.find(" IS ") else { return };
    let after_is = is_idx + 4;
    let mut i = after_is;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
      i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'\'' {
      return;
    }
    // Read until next `'` (no escapes for this quick check).
    let str_start = i + 1;
    let mut j = str_start;
    while j < bytes.len() && bytes[j] != b'\'' {
      j += 1;
    }
    let content = &body[str_start..j];
    if content.trim().is_empty() {
      let abs_start = start + str_start - 1; // include opening `'`
      let abs_end = start + j + 1; // include closing `'`
      out.push(Diagnostic {
        code: "sql091",
        severity: Severity::Hint,
        message: "empty COMMENT -- either delete the statement or fill in the description".into(),
        range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
      });
    }
  }
}
