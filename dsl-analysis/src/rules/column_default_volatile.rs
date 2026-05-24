//! sql145: column `DEFAULT now()` (or any volatile expression) freezes
//! the value at insert time, which is usually fine -- but DEFAULT
//! random() / nextval() / etc. inside CREATE TABLE produces a fresh
//! value per row at insert. Surface as a Hint so the user is aware
//! the default is recomputed per row.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql145" }
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
        if !(trimmed.starts_with("CREATE TABLE") || trimmed.starts_with("ALTER TABLE")) { return; }
        let bytes = upper.as_bytes();
        let n = bytes.len();
        let mut i = 0;
        while i + 7 <= n {
            if &upper[i..i + 7] == "DEFAULT"
                && (i == 0 || !is_word(bytes[i - 1] as char))
                && (i + 7 == n || !is_word(bytes[i + 7] as char))
            {
                let mut j = i + 7;
                while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                let arg_start = j;
                while j < n && (is_word(bytes[j] as char) || bytes[j] == b'(' || bytes[j] == b')') {
                    j += 1;
                }
                let arg = &upper[arg_start..j];
                let volatile = arg.starts_with("RANDOM(")
                    || arg.starts_with("NEXTVAL(")
                    || arg.starts_with("GEN_RANDOM_UUID(")
                    || arg.starts_with("UUID_GENERATE")
                    || arg.starts_with("CLOCK_TIMESTAMP(");
                if volatile {
                    let abs_start = start + i;
                    let abs_end = start + j;
                    out.push(Diagnostic {
                        code: "sql145",
                        severity: Severity::Hint,
                        message: format!("DEFAULT `{}` is volatile -- the column gets a fresh value per inserted row, not a single fixed value", &body[arg_start..j]),
                        range: text_size::TextRange::new(
                            (abs_start as u32).into(),
                            (abs_end as u32).into(),
                        ),
                    });
                    return;
                }
                i = j;
                continue;
            }
            i += 1;
        }
    }
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }
