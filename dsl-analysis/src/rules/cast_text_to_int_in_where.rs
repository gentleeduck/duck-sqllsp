//! sql121: comparing a text expression to an int literal in WHERE.
//! Common bug -- PG will cast text -> int row-by-row and discard the
//! index. Catches `t.id_text = 123` style patterns where the left side
//! is wrapped in a text function.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql121"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    // Pattern: `(TEXT|VARCHAR|CHAR) ... = <digit>` -- look for an
    // explicit ::text cast immediately left of `=`, with a numeric
    // RHS.
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 6 <= n {
      // Find `::text` / `::varchar` / `::char` before `=`.
      if i + 6 <= n && &upper[i..i + 6] == "::TEXT" {
        let after = i + 6;
        let mut j = after;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && bytes[j] == b'=' {
          let mut k = j + 1;
          while k < n && bytes[k].is_ascii_whitespace() {
            k += 1;
          }
          if k < n && (bytes[k].is_ascii_digit() || bytes[k] == b'-') {
            let abs_start = start + i;
            let abs_end = start + j + 1;
            out.push(Diagnostic {
                            code: "sql121",
                            severity: Severity::Hint,
                            message: "cast to text compared to numeric literal -- the cast disables index use; compare on the numeric column directly".into(),
                            range: crate::range_at(abs_start, abs_end),
                        });
            return;
          }
        }
      }
      i += 1;
    }
  }
}
