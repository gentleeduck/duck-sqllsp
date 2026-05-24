//! sql113: `TIMESTAMP` without time zone -- ambiguous across sessions.
//! Prefer `TIMESTAMPTZ` (`TIMESTAMP WITH TIME ZONE`).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql113" }
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
        // Only inspect column-typing contexts.
        if !upper.contains("CREATE TABLE") && !upper.contains("ALTER TABLE")
            && !upper.contains("CAST(") && !upper.contains("::") {
            return;
        }
        let bytes = upper.as_bytes();
        let n = bytes.len();
        let mut i = 0;
        while i + 9 <= n {
            if &upper[i..i + 9] == "TIMESTAMP"
                && (i == 0 || !is_word(bytes[i - 1] as char))
                && (i + 9 == n || !is_word(bytes[i + 9] as char))
            {
                // Skip TIMESTAMPTZ itself.
                let mut j = i + 9;
                while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                // `TIMESTAMP WITH ...`?
                if j + 4 <= n && &upper[j..j + 4] == "WITH" {
                    // TIMESTAMP WITH TIME ZONE -- fine.
                    i = j + 4; continue;
                }
                // `TIMESTAMP WITHOUT TIME ZONE` -- still flag.
                let abs_start = start + i;
                let abs_end = start + i + 9;
                out.push(Diagnostic {
                    code: "sql113",
                    severity: Severity::Hint,
                    message: "TIMESTAMP without time zone is ambiguous -- prefer TIMESTAMPTZ".into(),
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
