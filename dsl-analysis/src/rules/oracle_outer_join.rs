//! sql326: `a.id = b.id(+)` -- Oracle's pre-ANSI outer-join hint.
//! PG uses ANSI `LEFT JOIN` / `RIGHT JOIN` instead.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql326"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
      if bytes[i] == b'\'' {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
        if i < bytes.len() { i += 1 }
        continue;
      }
      if bytes[i] == b'(' && i + 2 < bytes.len() && bytes[i + 1] == b'+' && bytes[i + 2] == b')' {
        // Must be preceded by an identifier (col(+)).
        if i > 0 {
          let prev = bytes[i - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' || prev == '"' {
            let abs_s = start + i;
            let abs_e = abs_s + 3;
            out.push(Diagnostic {
              code: "sql326",
              severity: Severity::Error,
              message: "`(+)` outer-join hint is Oracle pre-ANSI syntax -- PG uses explicit `LEFT JOIN` / `RIGHT JOIN`".into(),
              range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
            i += 3;
            continue;
          }
        }
      }
      i += 1;
    }
  }
}
