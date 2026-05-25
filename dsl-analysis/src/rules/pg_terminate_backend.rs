//! sql332: `pg_terminate_backend(...)` / `pg_cancel_backend(...)`
//! invoked from an unprivileged buffer. PG requires the caller to be
//! a superuser (or have `pg_signal_backend` on PG13+). Useful to flag
//! because the failure mode is silent (function returns `false`).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql332"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    for needle in ["PG_TERMINATE_BACKEND(", "PG_CANCEL_BACKEND("] {
      if let Some(at) = upper.find(needle) {
        let abs_s = start + at;
        let abs_e = abs_s + needle.len() - 1; // exclude '('
        out.push(Diagnostic {
          code: "sql332",
          severity: Severity::Warning,
          message: format!(
            "`{}` requires superuser / pg_signal_backend privilege; returns false silently if denied",
            &needle[..needle.len() - 1]
          ),
          range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
        return;
      }
    }
  }
}
