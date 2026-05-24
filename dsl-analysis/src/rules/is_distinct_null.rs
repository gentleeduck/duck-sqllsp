//! sql095: `x IS NOT DISTINCT FROM NULL` is just `x IS NULL`; the
//! other form is `x IS DISTINCT FROM NULL` ≡ `x IS NOT NULL`. Both
//! confuse readers -- suggest the shorter form.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql095" }
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
        let bytes = upper.as_bytes();
        let n = bytes.len();
        let mut i = 0;
        while i + 2 <= n {
            // Look for "IS [NOT] DISTINCT FROM NULL".
            if &upper[i..i + 2] == "IS"
                && (i == 0 || !is_word(bytes[i - 1] as char))
            {
                let mut j = i + 2;
                while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                let mut not = false;
                if j + 3 <= n && &upper[j..j + 3] == "NOT" {
                    not = true;
                    j += 3;
                    while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                }
                if j + 8 > n || &upper[j..j + 8] != "DISTINCT" { i += 1; continue; }
                j += 8;
                while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                if j + 4 > n || &upper[j..j + 4] != "FROM" { i += 1; continue; }
                j += 4;
                while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                if j + 4 > n || &upper[j..j + 4] != "NULL" { i += 1; continue; }
                let next_ok = j + 4 == n || !is_word(bytes[j + 4] as char);
                if !next_ok { i += 1; continue; }
                let abs_start = start + i;
                let abs_end = start + j + 4;
                let suggest = if not { "IS NULL" } else { "IS NOT NULL" };
                out.push(Diagnostic {
                    code: "sql095",
                    severity: Severity::Hint,
                    message: format!("rewrite as `{suggest}` -- shorter and clearer than DISTINCT FROM NULL"),
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
