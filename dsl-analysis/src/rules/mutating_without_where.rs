//! sql013: UPDATE or DELETE without a WHERE clause. Almost always a bug
//! waiting to clear out a whole table.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql013" }
    fn default_severity(&self) -> Severity { Severity::Warning }

    fn check(
        &self,
        _source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        match &stmt.kind {
            StatementKind::Update(u) if u.where_clause.is_none() => {
                out.push(Diagnostic {
                    code: "sql013",
                    severity: Severity::Warning,
                    message: format!(
                        "UPDATE without WHERE will modify every row in `{}`",
                        u.table.name
                    ),
                    range: stmt.range,
                });
            }
            StatementKind::Delete(d) if d.where_clause.is_none() => {
                out.push(Diagnostic {
                    code: "sql013",
                    severity: Severity::Warning,
                    message: format!(
                        "DELETE without WHERE will remove every row in `{}`",
                        d.table.name
                    ),
                    range: stmt.range,
                });
            }
            _ => {}
        }
    }
}
