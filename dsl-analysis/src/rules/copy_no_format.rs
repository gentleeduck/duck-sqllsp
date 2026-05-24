//! sql136: `COPY t FROM 'file'` without a `FORMAT` clause -- defaults
//! to `text` which has subtle escaping rules. Hint to make the format
//! explicit.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql136" }
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
        if !trimmed.starts_with("COPY ") { return; }
        // If FORMAT or CSV/TEXT/BINARY appears anywhere in the stmt, skip.
        if upper.contains("FORMAT")
            || upper.contains(" CSV")
            || upper.contains(" TEXT ")
            || upper.contains(" BINARY")
        { return; }
        let leading = upper.len() - trimmed.len();
        let abs_start = start + leading;
        let abs_end = abs_start + 4;
        out.push(Diagnostic {
            code: "sql136",
            severity: Severity::Hint,
            message: "COPY without an explicit FORMAT clause defaults to `text` -- spell it (`WITH (FORMAT csv, ...)`) so the file shape is unambiguous".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
