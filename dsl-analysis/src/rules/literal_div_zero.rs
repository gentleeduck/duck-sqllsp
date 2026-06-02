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
    let (start, body) = crate::stmt_body(stmt, source);
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
      if bytes[i] == b'\'' {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
        if i < bytes.len() {
          i += 1
        }
        continue;
      }
      // Division `/ 0` and modulo `% 0` -- both raise 22012 in PG.
      let is_div = bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] != b'/' && bytes[i + 1] != b'*';
      let is_mod = bytes[i] == b'%';
      if is_div || is_mod {
        let op_char = bytes[i] as char;
        let mut k = i + 1;
        while k < bytes.len() && bytes[k].is_ascii_whitespace() {
          k += 1
        }
        let num_start = k;
        while k < bytes.len() && (bytes[k].is_ascii_digit() || bytes[k] == b'.') {
          k += 1
        }
        if k > num_start {
          let lit = &body[num_start..k];
          if let Ok(v) = lit.parse::<f64>()
            && v == 0.0
          {
            let label = if is_div { "division" } else { "modulo" };
            out.push(Diagnostic {
              code: "sql278",
              severity: Severity::Error,
              message: format!("Literal {label} by zero `{op_char} {lit}` -- PG raises 22012 at runtime"),
              range: crate::range_at(start + i, start + k),
            });
          }
        }
        i = k;
        continue;
      }
      i += 1;
    }
  }
}
