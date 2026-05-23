//! sql003: unqualified column reference exists in more than one in-scope
//! table; the user must qualify it.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Expr, Projection, SelectStmt, Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql003" }
    fn default_severity(&self) -> Severity { Severity::Error }

    fn check(
        &self,
        _source: &str,
        stmt: &Statement,
        scope: &Scope,
        catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        if scope.is_empty() || catalog.tables().next().is_none() { return; }
        let StatementKind::Select(s) = &stmt.kind else { return; };
        let mut refs: Vec<(Option<String>, String)> = Vec::new();
        collect(s, &mut refs);

        for (qualifier, name) in refs {
            if qualifier.is_some() { continue; } // Already qualified.
            // Dedup by underlying table: dsl-resolve binds each table
            // twice (once by alias, once by bare name) so we'd otherwise
            // report `[u, u, o, o]` instead of `[u, o]`.
            let mut hits: Vec<String> = Vec::new();
            let mut seen_tables: Vec<(Option<String>, String)> = Vec::new();
            for b in scope.tables() {
                let key = (b.table.schema.clone(), b.table.name.clone());
                if seen_tables.contains(&key) { continue; }
                seen_tables.push(key);
                if let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) {
                    if t.columns.iter().any(|c| c.name == name) {
                        hits.push(b.alias.clone());
                    }
                }
            }
            if hits.len() > 1 {
                out.push(Diagnostic {
                    code: "sql003",
                    severity: Severity::Error,
                    message: format!(
                        "ambiguous column `{name}`: appears in [{}]; qualify with the table alias",
                        hits.join(", ")
                    ),
                    range: stmt.range,
                });
            }
        }
    }
}

fn collect(s: &SelectStmt, out: &mut Vec<(Option<String>, String)>) {
    for p in &s.projections {
        if let Projection::Expr { expr, .. } = p {
            walk(expr, out);
        }
    }
    if let Some(w) = &s.where_clause { walk(w, out); }
    for j in &s.joins {
        if let Some(on) = &j.on { walk(on, out); }
    }
}

fn walk(e: &Expr, out: &mut Vec<(Option<String>, String)>) {
    match e {
        Expr::Column { qualifier, name, .. } => out.push((qualifier.clone(), name.clone())),
        Expr::BinaryOp { left, right, .. } => { walk(left, out); walk(right, out); }
        Expr::Call { args, .. } => { for a in args { walk(a, out); } }
        _ => {}
    }
}
