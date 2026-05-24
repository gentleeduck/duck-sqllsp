//! sql169: `ALTER TABLE x OWNER TO some_role` -- when a live catalog
//! is connected, validate that `some_role` exists in `pg_roles`.
//! Otherwise silently runs and PG errors at exec.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql169" }
    fn default_severity(&self) -> Severity { Severity::Error }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        // No catalog roles loaded -> can't validate. Silent skip is
        // intentional: better than guessing or false-flagging when the
        // user is editing without a DB connection.
        if catalog.roles.is_empty() { return; }
        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let upper = body.to_ascii_uppercase();
        // Look for `OWNER TO <ident>`.
        let Some(rel) = upper.find("OWNER TO") else { return };
        let bytes = body.as_bytes();
        let mut j = rel + 8;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() { j += 1; }
        // Optional CURRENT_USER / SESSION_USER built-ins -- fine.
        for kw in ["CURRENT_USER", "SESSION_USER", "CURRENT_ROLE"] {
            if j + kw.len() <= bytes.len()
                && upper[j..j + kw.len()].eq_ignore_ascii_case(kw)
            {
                return;
            }
        }
        // Quoted identifier or bare identifier.
        let (role_start, role_end) = if j < bytes.len() && bytes[j] == b'"' {
            let q_start = j + 1;
            let mut k = q_start;
            while k < bytes.len() && bytes[k] != b'"' { k += 1; }
            (q_start, k)
        } else {
            let id_start = j;
            let mut k = id_start;
            while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
                k += 1;
            }
            (id_start, k)
        };
        if role_end == role_start { return; }
        let role = &body[role_start..role_end];
        // Catalog stores names case-sensitive; PG identifiers are
        // case-folded to lowercase unless quoted.
        let role_norm = role.to_ascii_lowercase();
        if catalog.roles.iter().any(|r| r.eq_ignore_ascii_case(&role_norm)) {
            return;
        }
        let abs_start = start + role_start;
        let abs_end = start + role_end;
        out.push(Diagnostic {
            code: "sql169",
            severity: Severity::Error,
            message: format!(
                "unknown role `{role}` -- not found in pg_roles; ALTER TABLE will fail at execution"
            ),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
