//! sql098: more than one `WHERE` clause in the same statement (outside
//! parentheses/subqueries). Usually a copy/paste mistake -- PG rejects
//! at parse time.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql098" }
    fn default_severity(&self) -> Severity { Severity::Error }

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
        let bytes = body.as_bytes();
        let ubytes = upper.as_bytes();
        let n = bytes.len();
        let mut depth = 0i32;
        let mut first: Option<usize> = None;
        let mut i = 0;
        while i < n {
            match bytes[i] {
                b'(' => { depth += 1; i += 1; continue; }
                b')' => { depth -= 1; i += 1; continue; }
                b'\'' => {
                    i += 1;
                    while i < n && bytes[i] != b'\'' { i += 1; }
                    if i < n { i += 1; }
                    continue;
                }
                _ => {}
            }
            if depth == 0 && i + 5 <= n && &upper[i..i + 5] == "WHERE" {
                let prev_ok = i == 0 || !is_word(ubytes[i - 1] as char);
                let next_ok = i + 5 == n || !is_word(ubytes[i + 5] as char);
                if prev_ok && next_ok {
                    match first {
                        None => { first = Some(i); }
                        Some(_) => {
                            let abs_start = start + i;
                            let abs_end = start + i + 5;
                            out.push(Diagnostic {
                                code: "sql098",
                                severity: Severity::Error,
                                message: "duplicate top-level WHERE clause -- did you mean AND/OR?".into(),
                                range: text_size::TextRange::new(
                                    (abs_start as u32).into(),
                                    (abs_end as u32).into(),
                                ),
                            });
                            return;
                        }
                    }
                    i += 5;
                    continue;
                }
            }
            i += 1;
        }
    }
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }
