//! sql090: PG 17 added `GROUP BY ALL` shorthand. Flag it as a Hint so
//! callers know about the portability cost (works only on PG 17+).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql090" }
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
        while i + 8 <= n {
            if &upper[i..i + 8] == "GROUP BY"
                && (i == 0 || !is_word(bytes[i - 1] as char))
            {
                let mut j = i + 8;
                while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                if j + 3 <= n && &upper[j..j + 3] == "ALL" {
                    let next_ok = j + 3 == n || !is_word(bytes[j + 3] as char);
                    if next_ok {
                        let abs_start = start + i;
                        let abs_end = start + j + 3;
                        out.push(Diagnostic {
                            code: "sql090",
                            severity: Severity::Hint,
                            message: "GROUP BY ALL requires PostgreSQL 17+ -- consider listing columns explicitly for portability".into(),
                            range: text_size::TextRange::new(
                                (abs_start as u32).into(),
                                (abs_end as u32).into(),
                            ),
                        });
                        return;
                    }
                }
            }
            i += 1;
        }
    }
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }
