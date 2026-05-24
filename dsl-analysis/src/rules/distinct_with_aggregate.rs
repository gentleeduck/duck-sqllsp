//! sql093: `SELECT DISTINCT count(...) FROM t` -- DISTINCT after an
//! aggregate without GROUP BY is almost always redundant or wrong.
//! Aggregates already collapse rows; DISTINCT on a single-row result
//! does nothing.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql093" }
    fn default_severity(&self) -> Severity { Severity::Warning }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        if !matches!(stmt.kind, StatementKind::Select(_)) { return; }
        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let upper = body.to_ascii_uppercase();
        if !upper.contains("SELECT DISTINCT") { return; }
        if upper.contains("GROUP BY") { return; }
        // Look for any aggregate function call.
        const AGGS: &[&str] = &[
            "COUNT(", "SUM(", "AVG(", "MIN(", "MAX(",
            "ARRAY_AGG(", "STRING_AGG(", "JSON_AGG(", "JSONB_AGG(",
            "BOOL_AND(", "BOOL_OR(", "EVERY(",
        ];
        if !AGGS.iter().any(|a| upper.contains(a)) { return; }
        let Some(distinct_pos) = upper.find("DISTINCT") else { return };
        let abs_start = start + distinct_pos;
        let abs_end = abs_start + 8;
        out.push(Diagnostic {
            code: "sql093",
            severity: Severity::Warning,
            message: "SELECT DISTINCT with an aggregate but no GROUP BY -- DISTINCT is redundant on collapsed rows".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
