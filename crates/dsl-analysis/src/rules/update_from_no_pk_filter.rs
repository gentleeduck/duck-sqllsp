//! sql127: `UPDATE t SET ... FROM other` without a WHERE that joins
//! `t` and `other` -- the FROM becomes a cross product and every row
//! of `t` gets touched.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql127" }
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
        let trimmed = upper.trim_start();
        if !trimmed.starts_with("UPDATE ") { return; }
        // Look for `FROM` keyword.
        if !upper.contains(" FROM ") { return; }
        let Some(from_at) = upper.find(" FROM ") else { return };
        // Must have WHERE after FROM, and that WHERE must mention `=`
        // tying two distinct identifiers (rough proxy for a join cond).
        let where_at = match upper[from_at..].find(" WHERE ") {
            Some(p) => from_at + p,
            None => {
                // No WHERE at all -- definitely cross-product.
                let leading = upper.len() - trimmed.len();
                let abs_start = start + leading;
                let abs_end = start + leading + 6;
                out.push(Diagnostic {
                    code: "sql127",
                    severity: Severity::Warning,
                    message: "UPDATE ... FROM without WHERE -- every row in the target table gets the cross-product".into(),
                    range: text_size::TextRange::new(
                        (abs_start as u32).into(),
                        (abs_end as u32).into(),
                    ),
                });
                return;
            }
        };
        let where_body = &body[where_at + 7..];
        // A heuristic join condition is `x.y = z.w` (two dotted names
        // on either side of `=`).
        let has_join_cond = where_body.contains('.') && where_body.contains('=');
        if has_join_cond { return; }
        let leading = upper.len() - trimmed.len();
        let abs_start = start + leading;
        let abs_end = start + leading + 6;
        out.push(Diagnostic {
            code: "sql127",
            severity: Severity::Warning,
            message: "UPDATE ... FROM ... WHERE without a join condition tying the two tables -- every row in the target table gets the cross-product".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
