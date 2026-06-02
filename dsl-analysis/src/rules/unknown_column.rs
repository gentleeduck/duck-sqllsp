//! sql002: column reference does not exist in any in-scope table.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Expr, Projection, SelectStmt, Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql002"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, _source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Need at least one in-scope table to resolve columns against.
    if scope.is_empty() {
      return;
    }
    // Catalog might be empty (no connection yet).
    if catalog.tables().next().is_none() {
      return;
    }

    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
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
        let range = if col_range.len() > text_size::TextSize::from(0) { col_range } else { stmt.range };
        out.push(Diagnostic { code: "sql002", severity: Severity::Error, message: msg, range });
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
      if cl == lower {
        return Some(c.name.clone());
      }
      let score = if cl.starts_with(&lower) || lower.starts_with(&cl) {
        1
      } else if cl.contains(&lower) || lower.contains(&cl) {
        2
      } else {
        // Levenshtein distance; accept when distance <= 2.
        let d = levenshtein(&cl, &lower);
        if d <= 2 {
          3 + d
        } else {
          continue;
        }
      };
      match &best {
        None => best = Some((score, c.name.clone())),
        Some((s, _)) if score < *s => best = Some((score, c.name.clone())),
        _ => {},
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
  if m == 0 {
    return n;
  }
  if n == 0 {
    return m;
  }
  let mut prev: Vec<usize> = (0..=n).collect();
  let mut curr = vec![0usize; n + 1];
  for i in 1..=m {
    curr[0] = i;
    for j in 1..=n {
      let cost = if av[i - 1] == bv[j - 1] { 0 } else { 1 };
      curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
    }
    std::mem::swap(&mut prev, &mut curr);
  }
  prev[n]
}

/// System schemas we never introspect (PG's planner-internal catalog +
/// the SQL-standard read-only views). When any in-scope binding targets
/// one of these, we cannot enumerate its columns -- be lenient and
/// accept anything rather than crying wolf on `information_schema.tables
/// .table_schema`, `pg_catalog.pg_class.oid`, etc.
fn is_system_schema(s: &str) -> bool {
  matches!(s.to_ascii_lowercase().as_str(), "information_schema" | "pg_catalog" | "pg_toast")
}

/// PG system columns that exist implicitly on every table (`pg_attribute`
/// "system attributes") but are NOT part of the column list in the live
/// or workspace catalog. Treating any of these as unknown produces a
/// false positive for legitimate queries like `SELECT ctid, xmin FROM t`.
fn is_pg_system_column(s: &str) -> bool {
  matches!(s.to_ascii_lowercase().as_str(), "ctid" | "xmin" | "xmax" | "cmin" | "cmax" | "tableoid" | "oid")
}

pub(crate) fn column_exists(scope: &Scope, catalog: &Catalog, qualifier: Option<&str>, name: &str) -> bool {
  // PG system columns (ctid, xmin, xmax, cmin, cmax, tableoid, oid)
  // are implicit on every table; never flag them as unknown.
  if is_pg_system_column(name) {
    return true;
  }
  if let Some(q) = qualifier {
    // Qualifier names a system schema directly (`pg_catalog.pg_class`,
    // `information_schema.tables`): we don't introspect those, so any
    // column reference is accepted.
    if let Some((schema, _)) = q.split_once('.')
      && is_system_schema(schema)
    {
      return true;
    }
    // Schema-qualified qualifier `schema.table` -- look up directly
    // in the catalog before falling back to scope/CTE lookups.
    if let Some((schema, table)) = q.split_once('.')
      && let Some(t) = catalog.find_table(Some(schema), table)
    {
      return t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(name));
    }
    // Qualifier matches a CTE name? Check declared CTE columns.
    // Empty Vec means the resolver could not parse the body --
    // be lenient and accept the column rather than emit a false
    // positive.
    if let Some(cte_cols) = scope.cte_columns_of(q) {
      if cte_cols.is_empty() {
        return true;
      }
      return cte_cols.iter().any(|c| c == name);
    }
    if let Some(b) = scope.get(q) {
      // Synthetic binding (function-call / subquery / CTE alias) --
      // we can't enumerate columns, so accept anything.
      if b.table.schema.as_deref().is_some_and(|s| s.starts_with('<')) {
        return true;
      }
      // Binding points at a system schema we don't introspect
      // (`pg_catalog`, `information_schema`, `pg_toast`). Resolve
      // anything leniently -- columns of system catalogs aren't in
      // the offline catalog and false-firing on `c.relname` after
      // `FROM pg_catalog.pg_class c` is louder than the missed typo.
      if b.table.schema.as_deref().is_some_and(is_system_schema) {
        return true;
      }
      // Follow alias to underlying CTE: `WITH foo AS (...) SELECT a.col FROM foo a`
      // -- qualifier `a` resolves to binding{table=foo}; check cte_columns_of("foo").
      if let Some(cte_cols) = scope.cte_columns_of(&b.table.name) {
        if cte_cols.is_empty() {
          return true;
        }
        return cte_cols.iter().any(|c| c == name);
      }
      if let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) {
        return t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(name));
      }
    }
    // Unknown qualifier: most likely an outer-scope binding from a
    // correlated subquery (`EXISTS (SELECT ... WHERE inner.x = outer.y)`)
    // that the per-statement resolver can't see. False-firing here is
    // worse than missing a typo, so accept the column. Keeps sql001
    // (unresolved table) as the canonical check for "alias doesn't
    // exist at all".
    return true;
  }
  // Unqualified column: check catalog tables in scope and CTE columns.
  // Also accept when `name` matches a binding's alias / name -- function
  // call FROM sources (`generate_series(...) AS number`) and subquery
  // aliases bind under that name and their "column" IS the alias.
  if scope.get(name).is_some() {
    return true;
  }
  // Lenient when any in-scope binding is synthetic (function-call FROM
  // `<func>`, subquery alias `<subq>`) OR targets a system schema we
  // don't introspect (`information_schema`, `pg_catalog`): we cannot
  // enumerate the source's columns reliably, so an unqualified
  // reference may legitimately resolve against it. Better silent than
  // crying wolf.
  for b in scope.tables() {
    if b.table.schema.as_deref().is_some_and(|s| s.starts_with('<') || is_system_schema(s)) {
      return true;
    }
  }
  for b in scope.tables() {
    if let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name)
      && t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(name))
    {
      return true;
    }
  }
  for (cte_name, cols) in &scope.cte_columns {
    // Only consider CTEs that are also bound as a FROM target.
    if scope.get(cte_name).is_none() {
      continue;
    }
    if cols.is_empty() {
      return true;
    }
    if cols.iter().any(|c| c == name) {
      return true;
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
  if let Some(w) = &s.where_clause {
    walk(w, out);
  }
  for j in &s.joins {
    if let Some(on) = &j.on {
      walk(on, out);
    }
  }
}

fn walk(e: &Expr, out: &mut Vec<(Option<String>, String, TextRange)>) {
  match e {
    Expr::Column { qualifier, name, range } => {
      out.push((qualifier.clone(), name.clone(), *range));
    },
    Expr::BinaryOp { left, right, .. } => {
      walk(left, out);
      walk(right, out);
    },
    Expr::Call { args, .. } => {
      for a in args {
        walk(a, out);
      }
    },
    Expr::List(items) => {
      for it in items {
        walk(it, out);
      }
    },
    _ => {},
  }
}
