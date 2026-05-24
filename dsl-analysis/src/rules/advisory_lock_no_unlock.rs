//! sql160: `pg_advisory_lock(...)` (session-level) without a matching
//! `pg_advisory_unlock(...)` in the same source. Session locks persist
//! beyond the transaction and leak across pool reuse.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql160"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(rel) = upper.find("PG_ADVISORY_LOCK(") else { return };
    // Transaction-scoped variant `pg_advisory_xact_lock` releases at
    // COMMIT/ROLLBACK -- not flagged.
    if upper[rel..].starts_with("PG_ADVISORY_XACT_LOCK") {
      return;
    }
    // Source-wide: does any later statement call pg_advisory_unlock
    // (matching key) or pg_advisory_unlock_all()?
    let after = &source[end..].to_ascii_uppercase();
    if after.contains("PG_ADVISORY_UNLOCK") {
      return;
    }
    let abs_start = start + rel;
    let abs_end = abs_start + 17;
    out.push(Diagnostic {
            code: "sql160",
            severity: Severity::Warning,
            message: "pg_advisory_lock() without a matching pg_advisory_unlock() -- session locks leak across pool reuse; use pg_advisory_xact_lock() to release at COMMIT".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
  }
}
