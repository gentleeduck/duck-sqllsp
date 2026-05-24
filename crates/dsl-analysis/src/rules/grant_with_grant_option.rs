//! sql133: `GRANT ... WITH GRANT OPTION` lets the grantee re-grant the
//! privilege chain to anyone else -- almost always too broad for
//! application roles.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql133" }
    fn default_severity(&self) -> Severity { Severity::Warning }

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
        if !trimmed.starts_with("GRANT ") { return; }
        let Some(at) = upper.find("WITH GRANT OPTION") else { return };
        let abs_start = start + at;
        let abs_end = abs_start + 17;
        out.push(Diagnostic {
            code: "sql133",
            severity: Severity::Warning,
            message: "WITH GRANT OPTION lets the grantee re-grant this privilege to others -- usually too broad for application roles".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
