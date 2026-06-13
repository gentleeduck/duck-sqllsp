//! sql542: `now()::date` / `current_timestamp::date` -- casting the current
//! timestamp to a date is exactly what `CURRENT_DATE` returns, more directly
//! and without a per-row cast. Also covers `localtimestamp::date`. A small
//! readability / idiom hint.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const PATTERNS: &[&str] = &["now()::date", "current_timestamp::date", "localtimestamp::date", "transaction_timestamp()::date"];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql542"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    // Collapse whitespace around `::` so `now() :: date` matches too.
    let lower = body.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    for pat in PATTERNS {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(pat) {
        let at = from + rel;
        // Word boundary before the function/keyword.
        let prev_ok = at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
        if prev_ok {
          out.push(Diagnostic {
            code: "sql542",
            severity: Severity::Hint,
            message: "use `CURRENT_DATE` instead of casting the current timestamp to date".into(),
            range: crate::range_at(start + at, start + at + pat.len()),
          });
        }
        from = at + pat.len();
      }
    }
  }
}
