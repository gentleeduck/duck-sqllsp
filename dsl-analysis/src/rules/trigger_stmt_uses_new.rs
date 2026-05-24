//! sql159: `CREATE TRIGGER ... FOR EACH STATEMENT ... NEW` -- only
//! row-level triggers have NEW/OLD. Statement-level triggers cannot
//! reference them.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql159" }
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
        if !upper.contains("CREATE TRIGGER") { return; }
        if !upper.contains("FOR EACH STATEMENT") { return; }
        // Look for NEW. / OLD. in any subsequent WHEN clause / function
        // body referenced. Best-effort: scan for bare NEW or OLD as a
        // word in the trigger source.
        let bytes = upper.as_bytes();
        let n = bytes.len();
        let mut i = 0;
        while i + 3 <= n {
            for kw in ["NEW", "OLD"] {
                if &upper[i..i + 3] == kw
                    && (i == 0 || !is_word(bytes[i - 1] as char))
                    && (i + 3 == n || !is_word(bytes[i + 3] as char))
                {
                    let abs_start = start + i;
                    let abs_end = start + i + 3;
                    out.push(Diagnostic {
                        code: "sql159",
                        severity: Severity::Error,
                        message: format!("FOR EACH STATEMENT trigger references {kw} -- only row-level (FOR EACH ROW) triggers have NEW/OLD"),
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
