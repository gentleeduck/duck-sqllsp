//! sql164: `'foo' || 1` or `'a' + 1` -- string literal + int. PG
//! requires explicit cast; the implicit one bites when porting from
//! MySQL.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql164"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
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
        // Walk to end of string literal.
        let lit_start = i;
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
        if i >= n {
          return;
        }
        let lit_end = i + 1;
        // Look at what follows the literal (skip ws).
        let mut j = lit_end;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j >= n {
          i = lit_end;
          continue;
        }
        // `+` / `-` followed by a digit triggers; `||` is fine
        // (concat). We don't flag `||`.
        if (bytes[j] == b'+' || bytes[j] == b'-') && j + 1 < n {
          let mut k = j + 1;
          while k < n && bytes[k].is_ascii_whitespace() {
            k += 1;
          }
          if k < n && bytes[k].is_ascii_digit() {
            let abs_start = start + lit_start;
            let abs_end = start + lit_end;
            out.push(Diagnostic {
                            code: "sql164",
                            severity: Severity::Hint,
                            message: "string literal in `+` / `-` with an integer -- PG requires an explicit cast; use `'x' || y::text` for concat or cast the literal".into(),
                            range: text_size::TextRange::new(
                                (abs_start as u32).into(),
                                (abs_end as u32).into(),
                            ),
                        });
            return;
          }
        }
        i = lit_end;
        continue;
      }
      i += 1;
    }
  }
}
