//! sql076: `LIMIT -1` / `OFFSET -1` -- PG rejects negative values.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql076"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    for kw in &["LIMIT", "OFFSET"] {
      if let Some(idx) = crate::textutil::find_word(&upper, kw) {
        let after = idx + kw.len();
        let ws = body[after..].len() - body[after..].trim_start().len();
        let num_start = after + ws;
        if body[num_start..].starts_with('-') {
          // Span from the `-` through the trailing digits so
          // the diagnostic underlines exactly the bad number.
          let mut num_end = num_start + 1;
          let bytes = body.as_bytes();
          while num_end < body.len() && (bytes[num_end].is_ascii_digit() || bytes[num_end] == b'.') {
            num_end += 1;
          }
          let abs_start = start + num_start;
          let abs_end = start + num_end;
          out.push(Diagnostic {
            code: "sql076",
            severity: Severity::Error,
            message: format!("{kw} cannot be negative"),
            range: crate::range_at(abs_start, abs_end),
          });
          return;
        }
      }
    }
  }
}
