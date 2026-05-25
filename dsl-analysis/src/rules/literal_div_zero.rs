//! sql278: `<expr> / 0` literal division by zero. PG raises 22012
//! at runtime. Catches the common typo / placeholder.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql278"
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
      if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] != b'/' && bytes[i + 1] != b'*' {
        let mut k = i + 1;
        while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1 }
        let num_start = k;
        while k < bytes.len() && (bytes[k].is_ascii_digit() || bytes[k] == b'.') { k += 1 }
        if k > num_start {
          let lit = &body[num_start..k];
          if let Ok(v) = lit.parse::<f64>() {
            if v == 0.0 {
              out.push(Diagnostic {
                code: "sql278",
                severity: Severity::Error,
                message: format!("Literal division by zero `/ {lit}` -- PG raises 22012 at runtime"),
                range: text_size::TextRange::new(((start + i) as u32).into(), ((start + k) as u32).into()),
              });
            }
          }
        }
        i = k;
        continue;
      }
      i += 1;
    }
  }
}
