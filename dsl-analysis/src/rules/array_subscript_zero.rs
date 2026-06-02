//! sql148: array subscript `arr[0]` or `arr[-1]` -- PG arrays are
//! 1-based by default. `arr[0]` returns NULL, never the first
//! element.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql148"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
      if bytes[i] == b'[' {
        // Skip if this is part of a type like `int[]` -- `[]` empty.
        let inner_start = i + 1;
        let mut j = inner_start;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && bytes[j] == b']' {
          i = j + 1;
          continue;
        }
        // Optional minus sign + digits.
        let lit_start = j;
        if j < n && bytes[j] == b'-' {
          j += 1;
        }
        let digits_start = j;
        while j < n && bytes[j].is_ascii_digit() {
          j += 1;
        }
        if j == digits_start {
          i += 1;
          continue;
        }
        let mut k = j;
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1;
        }
        if k >= n || bytes[k] != b']' {
          i += 1;
          continue;
        }
        let lit = &body[lit_start..j];
        let bad = matches!(lit.parse::<i64>(), Ok(v) if v <= 0);
        if bad {
          let abs_start = start + i;
          let abs_end = start + k + 1;
          out.push(Diagnostic {
            code: "sql148",
            severity: Severity::Warning,
            message: format!(
              "array subscript `[{lit}]` -- PG arrays are 1-based by default, `[0]` and negative indexes return NULL"
            ),
            range: crate::range_at(abs_start, abs_end),
          });
          return;
        }
        i = k + 1;
        continue;
      }
      i += 1;
    }
  }
}
