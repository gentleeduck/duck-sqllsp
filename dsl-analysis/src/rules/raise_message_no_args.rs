//! sql131: `RAISE NOTICE 'value is %s'` -- but no `,` providing the
//! argument. PG prints the placeholder as-is and probably swallows the
//! error if the user expected interpolation.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql131" }
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
        let bytes = body.as_bytes();
        let n = bytes.len();
        // Find each `RAISE <level> '...'` and check whether the string
        // literal contains a `%` placeholder and whether the rest of
        // the statement (up to `;`) has a comma after the literal.
        let mut i = 0;
        while i + 5 <= n {
            if &upper[i..i + 5] == "RAISE"
                && (i == 0 || !is_word(bytes[i - 1] as char))
            {
                let mut j = i + 5;
                while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                // Optional level keyword.
                for level in ["NOTICE", "WARNING", "EXCEPTION", "DEBUG", "INFO", "LOG"] {
                    if j + level.len() <= n && &upper[j..j + level.len()] == level
                        && (j + level.len() == n || !is_word(bytes[j + level.len()] as char))
                    {
                        j += level.len();
                        while j < n && bytes[j].is_ascii_whitespace() { j += 1; }
                        break;
                    }
                }
                if j >= n || bytes[j] != b'\'' { i += 1; continue; }
                let lit_start = j;
                j += 1;
                while j < n && bytes[j] != b'\'' { j += 1; }
                if j >= n { break; }
                let lit_end = j + 1;
                let lit = &body[lit_start..lit_end];
                // Count `%` placeholders, ignoring `%%`.
                let mut placeholders = 0;
                let lb = lit.as_bytes();
                let mut k = 0;
                while k < lb.len() {
                    if lb[k] == b'%' {
                        if k + 1 < lb.len() && lb[k + 1] == b'%' { k += 2; continue; }
                        placeholders += 1;
                    }
                    k += 1;
                }
                if placeholders == 0 { i = lit_end; continue; }
                // Look forward to `;` or `RAISE` and count top-level commas.
                let mut commas = 0;
                let mut m = lit_end;
                while m < n && bytes[m] != b';' {
                    if bytes[m] == b',' { commas += 1; }
                    if bytes[m] == b'\'' {
                        m += 1;
                        while m < n && bytes[m] != b'\'' { m += 1; }
                    }
                    m += 1;
                }
                if commas < placeholders {
                    let abs_start = start + lit_start;
                    let abs_end = start + lit_end;
                    out.push(Diagnostic {
                        code: "sql131",
                        severity: Severity::Warning,
                        message: format!("RAISE message has {placeholders} `%` placeholder(s) but only {commas} argument(s) supplied -- missing args render as literal text"),
                        range: text_size::TextRange::new(
                            (abs_start as u32).into(),
                            (abs_end as u32).into(),
                        ),
                    });
                    return;
                }
                i = lit_end;
                continue;
            }
            i += 1;
        }
    }
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }
