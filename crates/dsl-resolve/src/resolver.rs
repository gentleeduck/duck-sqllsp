//! Walk a parsed statement and produce its [`Scope`].
//!
//! Resolution is one pass: every FROM / JOIN reference adds a binding
//! under both its alias (if present) and its bare table name. Subsequent
//! lookups by either form resolve to the same row.

use crate::binding::Binding;
use crate::scope::Scope;
use dsl_parse::{Statement, StatementKind, TableRef};

/// Resolve every statement in `stmts`. Returns one [`Scope`] per statement,
/// in matching order, so callers can index by statement position.
pub fn resolve(stmts: &[Statement]) -> Vec<Scope> {
    stmts.iter().map(resolve_one).collect()
}

fn resolve_one(stmt: &Statement) -> Scope {
    let mut scope = Scope::default();
    match &stmt.kind {
        StatementKind::Select(s) => {
            // Bind CTE names first so they're visible to FROM lookups
            // when the same CTE appears later in the same SELECT.
            for name in &s.cte_names {
                add_synthetic(&mut scope, name);
                // Register the CTE in `cte_columns` with an empty Vec
                // -- the body parse that would fill projection columns
                // is a future enhancement. `Some(empty)` lets callers
                // tell "declared but unknown" from "no such CTE".
                scope.cte_columns.entry(name.clone()).or_insert_with(Vec::new);
            }
            for t in &s.from { add(&mut scope, t); }
            for j in &s.joins { add(&mut scope, &j.table); }
        }
        StatementKind::Update(u) => add(&mut scope, &u.table),
        StatementKind::Delete(d) => add(&mut scope, &d.table),
        StatementKind::Insert(i) => add(&mut scope, &i.table),
        _ => {}
    }
    scope
}

/// Bind a synthetic table reference (CTE / subquery alias). Columns of
/// the underlying body aren't resolved yet -- a future pass can promote
/// these from name-only to fully-typed bindings.
fn add_synthetic(scope: &mut Scope, name: &str) {
    if name.is_empty() { return; }
    let mut table = dsl_parse::TableRef::default();
    table.name = name.to_string();
    scope.bindings.entry(name.to_string()).or_insert(Binding {
        alias: name.to_string(),
        table,
    });
}

fn add(scope: &mut Scope, table: &TableRef) {
    if table.name.is_empty() {
        return;
    }
    let entry = Binding {
        alias: table
            .alias
            .clone()
            .unwrap_or_else(|| table.name.clone()),
        table: table.clone(),
    };
    if let Some(alias) = &table.alias {
        scope.bindings.insert(alias.clone(), entry.clone());
    }
    // Always also bind by the unaliased name so users can reference the
    // table without an alias inside the same query.
    scope.bindings.entry(table.name.clone()).or_insert(entry);
}
