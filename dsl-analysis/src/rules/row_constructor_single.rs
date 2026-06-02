//! sql166: `ROW(x)` with a single element -- PG treats it as a row,
//! but `(x)` is just `x`. Worth flagging when the user writes the
//! explicit ROW form with one element because it's almost always
//! pasted from a multi-element template.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql166"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 4 <= n {
      if &upper[i..i + 4] == "ROW(" && (i == 0 || !is_word(bytes[i - 1] as char)) {
        // Walk inside the parens, counting top-level commas.
        let inner_start = i + 4;
        let mut j = inner_start;
        let mut depth = 1i32;
        let mut commas = 0;
        while j < n && depth > 0 {
          match bytes[j] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 1 => commas += 1,
            b'\'' => {
              j += 1;
              while j < n && bytes[j] != b'\'' {
                j += 1;
              }
            },
            _ => {},
          }
          if depth == 0 {
            break;
          }
          j += 1;
        }
        if commas == 0 && j > inner_start + 1 {
          let abs_start = start + i;
          let abs_end = start + j + 1;
          out.push(Diagnostic {
                        code: "sql166",
                        severity: Severity::Hint,
                        message: "ROW(x) with a single element is just x -- drop the ROW() wrapper, or add more elements to make it a real row constructor".into(),
                        range: crate::range_at(abs_start, abs_end),
                    });
          return;
        }
        i = j.saturating_add(1);
        continue;
      }
      i += 1;
    }
  }
}

