//! sql123: `\n`, `\t`, `\\` inside a plain `'...'` string. PG 9.1+
//! defaults to `standard_conforming_strings = on` -- the backslash is
//! literal, not an escape. Use `E'...'` if the user wants escapes.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql123"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
      if bytes[i] == b'\'' {
        // E-prefixed string is fine.
        if i > 0 && (bytes[i - 1] == b'E' || bytes[i - 1] == b'e') {
          i += 1;
          while i < n && bytes[i] != b'\'' {
            i += 1;
          }
          if i < n {
            i += 1;
          }
          continue;
        }
        let str_start = i + 1;
        let mut j = str_start;
        while j < n && bytes[j] != b'\'' {
          if bytes[j] == b'\\' && j + 1 < n {
            let nxt = bytes[j + 1];
            if matches!(nxt, b'n' | b't' | b'r' | b'\\' | b'0' | b'x' | b'u') {
              let abs_start = start + j;
              let abs_end = start + j + 2;
              out.push(Diagnostic {
                                code: "sql123",
                                severity: Severity::Warning,
                                message: format!("backslash in plain string is literal (standard_conforming_strings) -- use E'...' if `\\{}` should be an escape", nxt as char),
                                range: text_size::TextRange::new(
                                    (abs_start as u32).into(),
                                    (abs_end as u32).into(),
                                ),
                            });
              return;
            }
          }
          j += 1;
        }
        i = if j < n { j + 1 } else { n };
        continue;
      }
      i += 1;
    }
  }
}
