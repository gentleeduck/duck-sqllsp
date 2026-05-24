//! sql116: bare `NUMERIC` / `DECIMAL` -- unbounded precision is fine
//! but rarely intentional. Most use-cases want NUMERIC(p,s).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql116" }
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
        if !upper.contains("CREATE TABLE") && !upper.contains("ALTER TABLE") { return; }
        let bytes = upper.as_bytes();
        let n = bytes.len();
        let mut i = 0;
        while i + 7 <= n {
            for (kw_len, kw) in &[(7usize, "NUMERIC"), (7, "DECIMAL")] {
                if i + kw_len <= n && &upper[i..i + kw_len] == *kw
                    && (i == 0 || !is_word(bytes[i - 1] as char))
                    && (i + kw_len == n || !is_word(bytes[i + kw_len] as char))
                {
                    // Skip if followed by `(`.
                    let mut j = i + kw_len;
                    while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                    if j < n && bytes[j] == b'(' { continue; }
                    let abs_start = start + i;
                    let abs_end = start + i + kw_len;
                    out.push(Diagnostic {
                        code: "sql116",
                        severity: Severity::Hint,
                        message: format!("{} without precision -- unbounded, prefer {}(p, s)", kw, kw),
                        range: text_size::TextRange::new(
                            (abs_start as u32).into(),
                            (abs_end as u32).into(),
                        ),
                    });
                    return;
                }
            }
            i += 1;
        }
    }
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }
