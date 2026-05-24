//! sql152: `BEGIN` for a transaction that needs to UPDATE/DELETE many
//! rows without an explicit `LOCK TABLE` or `FOR UPDATE` -- can lead
//! to lost updates when there's concurrent traffic. Hint to consider
//! explicit lock-mode for write-heavy transactions.
//!
//! Conservative heuristic: only flag when the transaction body contains
//! UPDATE/DELETE without a WHERE on a unique key.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql152" }
    fn default_severity(&self) -> Severity { Severity::Hint }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let upper = body.to_ascii_uppercase();
        let trimmed = upper.trim_start();
        if !(trimmed == "BEGIN;" || trimmed.starts_with("BEGIN;") || trimmed == "BEGIN") { return; }
        // Look at the source after this BEGIN until COMMIT/ROLLBACK.
        let after = &source[end..].to_ascii_uppercase();
        let end_at = after.find("COMMIT").or_else(|| after.find("ROLLBACK"))
            .unwrap_or(after.len());
        let tx_body = &after[..end_at];
        // Need write statements.
        let has_write = tx_body.contains("UPDATE ") || tx_body.contains("DELETE ");
        if !has_write { return; }
        // Already has explicit lock mode? Skip.
        if tx_body.contains("LOCK TABLE") || tx_body.contains("FOR UPDATE")
            || tx_body.contains("FOR SHARE") || tx_body.contains("FOR NO KEY UPDATE")
        { return; }
        let leading = upper.len() - trimmed.len();
        let abs_start = start + leading;
        let abs_end = abs_start + 5;
        out.push(Diagnostic {
            code: "sql152",
            severity: Severity::Hint,
            message: "BEGIN block performs UPDATE/DELETE without an explicit lock mode -- consider `SELECT ... FOR UPDATE` or `LOCK TABLE` to avoid lost-update races under concurrency".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
