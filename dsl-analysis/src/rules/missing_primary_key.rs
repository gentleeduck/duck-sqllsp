//! sql046: `CREATE TABLE` without a PRIMARY KEY.
//!
//! Heap tables without a primary key cause replication, ORM, and audit
//! pain. Warn so the author adds one explicitly (and suppresses with a
//! comment when intentionally omitting it -- e.g. log tables).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql046" }
    fn default_severity(&self) -> Severity { Severity::Warning }

    fn check(
        &self,
        source: &str,
        stmt: &Statement,
        _scope: &Scope,
        _catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        let StatementKind::CreateTable(ct) = &stmt.kind else { return; };
        let start: usize = u32::from(stmt.range.start()) as usize;
        let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
        let body = &source[start..end];
        let upper = body.to_ascii_uppercase();
        if upper.contains("PRIMARY KEY") { return; }
        if upper.contains("PARTITION OF ") { return; }
        if upper.contains("CREATE TEMP ") || upper.contains("CREATE TEMPORARY ") { return; }
        // Locate the table name token in the source. Prefer the parsed
        // TableRef.range when populated; otherwise text-scan.
        let range = if u32::from(ct.table.range.len()) > 0 {
            ct.table.range
        } else {
            find_table_name_range(body, start, &ct.table.name).unwrap_or(stmt.range)
        };
        out.push(Diagnostic {
            code: "sql046",
            severity: Severity::Warning,
            message: format!(
                "table `{}` has no PRIMARY KEY -- add one or mark as a log/audit table",
                ct.table.name,
            ),
            range,
        });
    }
}

fn find_table_name_range(
    body: &str,
    body_offset: usize,
    name: &str,
) -> Option<text_size::TextRange> {
    let upper = body.to_ascii_uppercase();
    for needle in ["CREATE TABLE IF NOT EXISTS ", "CREATE TEMPORARY TABLE ", "CREATE TEMP TABLE ", "CREATE TABLE "] {
        if let Some(idx) = upper.find(needle) {
            let after = idx + needle.len();
            let rest = &body[after..];
            let ws = rest.len() - rest.trim_start().len();
            let n_start = after + ws;
            let bytes = body.as_bytes();
            let mut e = n_start;
            while e < bytes.len()
                && (bytes[e].is_ascii_alphanumeric() || bytes[e] == b'_' || bytes[e] == b'.' || bytes[e] == b'"')
            {
                e += 1;
            }
            if body[n_start..e].eq_ignore_ascii_case(name) {
                return Some(text_size::TextRange::new(
                    ((body_offset + n_start) as u32).into(),
                    ((body_offset + e) as u32).into(),
                ));
            }
        }
    }
    None
}
