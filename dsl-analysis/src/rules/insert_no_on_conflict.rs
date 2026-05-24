//! sql083: `INSERT INTO t (id, ...)` referencing the primary key
//! without `ON CONFLICT` -- the second call with the same id fails.
//!
//! Hint: add `ON CONFLICT (id) DO NOTHING` or `ON CONFLICT (id) DO
//! UPDATE` when idempotency is desired.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql083" }
    fn default_severity(&self) -> Severity { Severity::Hint }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        let StatementKind::Insert(i) = &stmt.kind else { return; };
        if i.columns.is_empty() { return; }
        let Some(t) = catalog.find_table(i.table.schema.as_deref(), &i.table.name) else { return };
        // Find PK column names from constraints.
        let pk_cols: Vec<String> = t.constraints.iter()
            .filter(|c| matches!(c.kind, dsl_catalog::ConstraintKind::PrimaryKey))
            .flat_map(|c| c.columns.iter().cloned())
            .map(|s| s.to_ascii_lowercase())
            .collect();
        if pk_cols.is_empty() { return; }
        let inserts_pk = i.columns.iter().any(|c| pk_cols.contains(&c.to_ascii_lowercase()));
        if !inserts_pk { return; }

        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let upper = body.to_ascii_uppercase();
        if upper.contains("ON CONFLICT") { return; }
        let insert_pos = upper.find("INSERT").unwrap_or(0);
        let abs_start = start + insert_pos;
        let abs_end = abs_start + "INSERT".len();
        out.push(Diagnostic {
            code: "sql083",
            severity: Severity::Hint,
            message: "INSERT writes the primary key without ON CONFLICT -- consider `ON CONFLICT (pk) DO NOTHING/UPDATE` for idempotency".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
    }
}
