//! sql128: `GRANT ... TO PUBLIC` -- grants the privilege to *every*
//! current and future role. Almost always a mistake.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql128" }
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
        // Find `TO PUBLIC` (case-insensitive, word-bounded).
        let bytes = upper.as_bytes();
        let n = bytes.len();
        let mut i = 0;
        while i + 9 <= n {
            if &upper[i..i + 9] == "TO PUBLIC"
                && (i == 0 || !is_word(bytes[i - 1] as char))
                && (i + 9 == n || !is_word(bytes[i + 9] as char))
            {
                let abs_start = start + i;
                let abs_end = start + i + 9;
                out.push(Diagnostic {
                    code: "sql128",
                    severity: Severity::Warning,
                    message: "GRANT TO PUBLIC opens the privilege to every role -- target a specific role or group instead".into(),
                    range: text_size::TextRange::new(
                        (abs_start as u32).into(),
                        (abs_end as u32).into(),
                    ),
                });
                return;
            }
            i += 1;
        }
    }
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }
