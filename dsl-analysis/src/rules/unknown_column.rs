//! sql002: column reference does not exist in any in-scope table.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Expr, Projection, SelectStmt, Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
    fn code(&self) -> &'static str { "sql002" }
    fn default_severity(&self) -> Severity { Severity::Error }

    fn check(
        &self,
        _source: &str,
        stmt: &Statement,
        scope: &Scope,
        catalog: &Catalog,
        out: &mut Vec<Diagnostic>,
    ) {
        // Need at least one in-scope table to resolve columns against.
        if scope.is_empty() { return; }
        // Catalog might be empty (no connection yet).
        if catalog.tables().next().is_none() { return; }

        let StatementKind::Select(s) = &stmt.kind else { return; };
        let mut refs: Vec<(Option<String>, String, TextRange)> = Vec::new();
        collect_column_refs(s, &mut refs);

        for (qualifier, name, col_range) in refs {
            if !column_exists(scope, catalog, qualifier.as_deref(), &name) {
                let display = match &qualifier {
                    Some(q) => format!("{q}.{name}"),
                    None => name.clone(),
                };
                let suggestion = nearest_column(scope, catalog, &name);
                let msg = match suggestion {
                    Some(s) => format!("unknown column `{display}` — did you mean `{s}`?"),
                    None => format!("unknown column `{display}`"),
                };
                let range = if col_range.len() > text_size::TextSize::from(0) {
                    col_range
                } else {
                    stmt.range
                };
                out.push(Diagnostic {
                    code: "sql002",
                    severity: Severity::Error,
                    message: msg,
                    range,
                });
            }
        }
    }
}

fn nearest_column(scope: &Scope, catalog: &Catalog, wanted: &str) -> Option<String> {
    let lower = wanted.to_ascii_lowercase();
    let mut best: Option<(usize, String)> = None;
    for b in scope.tables() {
        let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) else {
            continue;
        };
        for c in &t.columns {
            let cl = c.name.to_ascii_lowercase();
            if cl == lower { return Some(c.name.clone()); }
            let score = if cl.starts_with(&lower) || lower.starts_with(&cl) { 1 }
                else if cl.contains(&lower) || lower.contains(&cl) { 2 }
                else {
                    // Levenshtein distance; accept when distance <= 2.
                    let d = levenshtein(&cl, &lower);
                    if d <= 2 { 3 + d } else { continue; }
                };
            match &best {
                None => best = Some((score, c.name.clone())),
                Some((s, _)) if score < *s => best = Some((score, c.name.clone())),
                _ => {}
            }
        }
    }
    best.map(|(_, n)| n)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    let m = av.len();
    let n = bv.len();
    if m == 0 { return n; }
    if n == 0 { return m; }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if av[i - 1] == bv[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

fn column_exists(
    scope: &Scope,
    catalog: &Catalog,
    qualifier: Option<&str>,
    name: &str,
) -> bool {
    if let Some(q) = qualifier {
        if let Some(b) = scope.get(q) {
            if let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) {
                return t.columns.iter().any(|c| c.name == name);
            }
        }
        return false;
    }
    for b in scope.tables() {
        if let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) {
            if t.columns.iter().any(|c| c.name == name) {
                return true;
            }
        }
    }
    false
}

fn collect_column_refs(s: &SelectStmt, out: &mut Vec<(Option<String>, String, TextRange)>) {
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

fn walk(e: &Expr, out: &mut Vec<(Option<String>, String, TextRange)>) {
    match e {
        Expr::Column { qualifier, name, range } => {
            out.push((qualifier.clone(), name.clone(), *range));
        }
        Expr::BinaryOp { left, right, .. } => { walk(left, out); walk(right, out); }
        Expr::Call { args, .. } => { for a in args { walk(a, out); } }
        _ => {}
    }
}
