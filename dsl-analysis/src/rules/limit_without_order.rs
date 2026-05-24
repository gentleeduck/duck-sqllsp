//! sql051: `LIMIT` without `ORDER BY` produces non-deterministic rows.
//!
//! PG's planner is free to return any subset matching the predicate
//! when no ORDER BY pins the row order. Warn so the author makes the
//! ordering explicit (or adds a comment if they really want the random
//! sample).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql051" }
    fn default_severity(&self) -> Severity { Severity::Warning }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        let StatementKind::Select(_) = &stmt.kind else { return; };
        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let upper = body.to_ascii_uppercase();
        if !contains_word(&upper, "LIMIT") { return; }
        if contains_word(&upper, "ORDER BY") { return; }
        // Single-row LIMIT 1 with a UNIQUE-ish predicate is often
        // intentional. Skip when LIMIT 1 appears (common pattern) --
        // but only that exact case.
        if upper.contains(" LIMIT 1") && !upper.contains(" LIMIT 10") {
            return;
        }
        // Narrow the diagnostic to the LIMIT keyword itself.
        let rel = upper.find("LIMIT").unwrap_or(0);
        let abs_start = start + rel;
        let abs_end = abs_start + 5;
        out.push(Diagnostic {
            code: "sql051",
            severity: Severity::Warning,
            message: "LIMIT without ORDER BY -- row selection is non-deterministic".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let n_bytes = needle.as_bytes();
    let mut i = 0;
    while i + n_bytes.len() <= bytes.len() {
        if &bytes[i..i + n_bytes.len()] == n_bytes {
            let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
            let next_ok = i + n_bytes.len() == bytes.len()
                || !is_word(bytes[i + n_bytes.len()] as char);
            if prev_ok && next_ok { return true; }
        }
        i += 1;
    }
    false
}
fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }
