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
        // Find the *last* DML statement-starter in the body. Word-bounded
        // so "new.upDATEd_at" isn't matched, and only at statement-start
        // positions (preceded by `;` / `BEGIN` / `THEN` / `LOOP` / start
        // of body) so that subqueries / CTE arms / `INSERT INTO ... ON
        // CONFLICT DO UPDATE` clauses don't trigger.
        let mut dml_at: Option<(usize, &'static str)> = None;
        for kw in ["UPDATE", "DELETE", "INSERT"] {
            let body_bytes = body_up.as_bytes();
            let n = body_up.len();
            let w = kw.len();
            let mut i = 0;
            while i + w <= n {
                if &body_up[i..i + w] == kw {
                    let prev_ok = i == 0 || !is_word(body_bytes[i - 1] as char);
                    let next_ok = i + w == n || !is_word(body_bytes[i + w] as char);
                    if prev_ok && next_ok && stmt_start(body_bytes, i) {
                        let new_pair = (i, kw);
                        dml_at = match dml_at {
                            None => Some(new_pair),
                            Some((prev, _)) if i > prev => Some(new_pair),
                            other => other,
                        };
                    }
                }
                i += 1;
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

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }

/// Is position `i` at the start of a statement -- i.e. preceded by
/// `;`, `BEGIN`, `THEN`, `ELSE`, `LOOP`, or only whitespace from the
/// beginning of the body?
fn stmt_start(bytes: &[u8], i: usize) -> bool {
    let mut k = i;
    while k > 0 && bytes[k - 1].is_ascii_whitespace() { k -= 1; }
    if k == 0 { return true; }
    if bytes[k - 1] == b';' { return true; }
    // Check word-bounded prior keyword.
    for kw in ["BEGIN", "THEN", "ELSE", "LOOP"] {
        let w = kw.len();
        if k >= w && bytes.get(k - w..k).map_or(false, |s| s.eq_ignore_ascii_case(kw.as_bytes())) {
            let pre_ok = k - w == 0 || !is_word(bytes[k - w - 1] as char);
            if pre_ok { return true; }
        }
    }
    false
}
