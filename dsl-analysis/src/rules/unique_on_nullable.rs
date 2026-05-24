//! sql139: `UNIQUE` on a nullable column with `NULLS DISTINCT` (the
//! PG default) -- multiple NULL rows are allowed. Usually surprising.
//! Suggest `UNIQUE NULLS NOT DISTINCT` (PG 15+) or making the column
//! `NOT NULL`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql139" }
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
        if !trimmed.starts_with("CREATE TABLE") { return; }
        let bytes = upper.as_bytes();
        let n = bytes.len();
        // Scan column definitions for `<col> <type> ... UNIQUE` without
        // NOT NULL and without NULLS NOT DISTINCT.
        let mut i = 0;
        while i + 6 <= n {
            if &upper[i..i + 6] == "UNIQUE"
                && (i == 0 || !is_word(bytes[i - 1] as char))
                && (i + 6 == n || !is_word(bytes[i + 6] as char))
            {
                // Walk back to the start of this column-definition line
                // (or the opening `(`) to read the type / NOT NULL.
                let mut k = i;
                while k > 0 && bytes[k - 1] != b',' && bytes[k - 1] != b'(' {
                    k -= 1;
                }
                let col_text = &upper[k..i];
                let has_not_null = col_text.contains("NOT NULL");
                if has_not_null { i += 6; continue; }
                // Already opted into NULLS NOT DISTINCT?
                let after = &upper[i + 6..];
                if after.trim_start().starts_with("NULLS NOT DISTINCT") { i += 6; continue; }
                let abs_start = start + i;
                let abs_end = start + i + 6;
                out.push(Diagnostic {
                    code: "sql139",
                    severity: Severity::Hint,
                    message: "UNIQUE on a nullable column -- multiple NULLs are allowed (default NULLS DISTINCT); add NOT NULL or `NULLS NOT DISTINCT` (PG 15+)".into(),
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
