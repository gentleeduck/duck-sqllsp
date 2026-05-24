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
        _source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        let StatementKind::Insert(i) = &stmt.kind else { return; };
        if !i.columns.is_empty() { return; }
        out.push(Diagnostic {
            code: "sql048",
            severity: Severity::Warning,
            message: format!(
                "INSERT INTO `{}` without column list -- positional VALUES break silently when the table schema changes",
                i.table.name,
            ),
            range: stmt.range,
        });
    }
}
