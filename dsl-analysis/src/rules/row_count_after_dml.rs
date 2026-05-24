//! sql126: DML inside a PL/pgSQL function without a subsequent
//! `GET DIAGNOSTICS rows = ROW_COUNT` -- callers usually want to know
//! whether the UPDATE/DELETE actually touched anything. Flag as Hint.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql126" }
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
        if !upper.contains("LANGUAGE PLPGSQL") { return; }
        // Look for the function body $$ ... $$.
        let Some(open) = body.find("$$") else { return };
        let Some(close_rel) = body[open + 2..].find("$$") else { return };
        let body_text = &body[open + 2..open + 2 + close_rel];
        let body_up = body_text.to_ascii_uppercase();
        // Find the *last* DML keyword in the body.
        let mut dml_at: Option<(usize, &'static str)> = None;
        for kw in ["UPDATE ", "DELETE ", "INSERT "] {
            if let Some(rel) = body_up.rfind(kw.trim()) {
                let new_pair = (rel, kw.trim());
                dml_at = match dml_at {
                    None => Some(new_pair),
                    Some((prev, _)) if rel > prev => Some(new_pair),
                    other => other,
                };
            }
        }
        let Some((dml_rel, kw)) = dml_at else { return };
        // If GET DIAGNOSTICS appears after the DML, OK.
        let after = &body_up[dml_rel..];
        if after.contains("GET DIAGNOSTICS") { return; }
        // If RETURNING ... INTO appears after the DML, also OK -- the
        // caller has a way to know.
        if after.contains("RETURNING") && after.contains(" INTO ") { return; }
        let abs_start = start + open + 2 + dml_rel;
        let abs_end = abs_start + kw.len();
        out.push(Diagnostic {
            code: "sql126",
            severity: Severity::Hint,
            message: format!("{kw} inside PL/pgSQL without a following `GET DIAGNOSTICS ... = ROW_COUNT` -- caller has no way to know whether any row was touched"),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
