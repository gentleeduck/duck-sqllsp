//! sql048: `INSERT INTO t VALUES (...)` without a column list.
//!
//! Positional INSERT works but is fragile -- adding or reordering
//! columns in the target table silently changes which column receives
//! which value. Warn to push users toward `INSERT INTO t (c1, c2)
//! VALUES (...)`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql048" }
    fn default_severity(&self) -> Severity { Severity::Warning }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        let StatementKind::Insert(i) = &stmt.kind else { return; };
        if !i.columns.is_empty() { return; }
        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let upper = body.to_ascii_uppercase();
        // Narrow to `INSERT INTO`.
        let (abs_start, abs_end) = upper.find("INSERT INTO")
            .map(|r| (start + r, start + r + 11))
            .unwrap_or((start, start + 6));
        out.push(Diagnostic {
            code: "sql048",
            severity: Severity::Warning,
            message: format!(
                "INSERT INTO `{}` without column list -- positional VALUES break silently when the table schema changes",
                i.table.name,
            ),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
