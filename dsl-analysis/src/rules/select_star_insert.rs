//! sql016: `INSERT INTO t SELECT *` is arity-fragile. A schema change to
//! the source table silently corrupts the destination. Always project
//! columns explicitly.
//!
//! Detection runs on the statement source slice because our Insert AST
//! does not carry the inner SELECT today.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql016" }
    fn default_severity(&self) -> Severity { Severity::Warning }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        if !matches!(stmt.kind, StatementKind::Insert(_)) { return; }

        let start: u32 = stmt.range.start().into();
        let end: u32 = stmt.range.end().into();
        let slice = &source[start as usize..end.min(source.len() as u32) as usize];
        let upper = slice.to_ascii_uppercase();

        // Quick text scan: must contain "SELECT" + "*" with no other
        // identifiers between them (cheap signal for `SELECT * FROM ...`).
        if let Some(sel) = upper.find("SELECT") {
            let after = &upper[sel + 6..];
            // Skip whitespace then check for `*` immediately.
            let trimmed = after.trim_start();
            if trimmed.starts_with('*') {
                out.push(Diagnostic {
                    code: "sql016",
                    severity: Severity::Warning,
                    message:
                        "INSERT ... SELECT * is fragile -- a column added to the source silently \
                         misaligns the destination. List the source columns explicitly."
                            .into(),
                    range: stmt.range,
                });
            }
        }
    }
}
