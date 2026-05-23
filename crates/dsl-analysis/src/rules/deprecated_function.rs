//! sql020: deprecated / non-recommended function call.
//!
//! These are calls Postgres still accepts but where the preferred form
//! differs. Surfaced as a Hint so it doesn't crowd real issues.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

/// Each entry is `(needle, message)`. Matching is case-insensitive and
/// requires the identifier to appear with `(` immediately after so we
/// don't flag a column called `substr`.
const REPLACEMENTS: &[(&str, &str)] = &[
    ("substr(", "`substr` is a Postgres alias; prefer the SQL-standard `substring(...)`."),
    ("array_length(",
     "`array_length(arr, 1)` returns NULL for empty arrays; consider `cardinality(arr)` for a 0-based count."),
];

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql020" }
    fn default_severity(&self) -> Severity { Severity::Hint }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        let range: TextRange = stmt.range;
        let start: u32 = range.start().into();
        let end: u32 = range.end().into();
        let slice = &source[start as usize..end.min(source.len() as u32) as usize];
        let upper = slice.to_ascii_uppercase();

        for (needle, msg) in REPLACEMENTS {
            if upper.contains(&needle.to_ascii_uppercase()) {
                out.push(Diagnostic {
                    code: "sql020",
                    severity: Severity::Hint,
                    message: (*msg).into(),
                    range,
                });
            }
        }
    }
}
