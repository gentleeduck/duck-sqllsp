//! sql081: `ORDER BY random()` -- slow, no index can help, runs a sort
//! over the entire result set. Hint: TABLESAMPLE BERNOULLI for sampling.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql081" }
    fn default_severity(&self) -> Severity { Severity::Hint }

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
        let Some(order_idx) = upper.find("ORDER BY") else { return };
        let after = order_idx + 8;
        let Some(rand_rel) = upper[after..].find("RANDOM(") else { return };
        let rand_pos = after + rand_rel;
        // Span through the matching close paren.
        let bytes = body.as_bytes();
        let mut depth = 1i32;
        let mut close = rand_pos + 7;
        while close < bytes.len() && depth > 0 {
            match bytes[close] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            close += 1;
        }
        let abs_start = start + rand_pos;
        let abs_end = start + close;
        out.push(Diagnostic {
            code: "sql081",
            severity: Severity::Hint,
            message: "ORDER BY random() sorts every row -- consider TABLESAMPLE for sampling, or pick a small subset first".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
