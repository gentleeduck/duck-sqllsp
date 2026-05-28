//! Postgres idioms that must NEVER false-positive on the
//! unknown-column / unresolved-table / unknown-function family
//! (`sql001`, `sql002`, `sql003`, `sql042`, `sql348`, `sql349`,
//! `sql350`, `sql351`).
//!
//! Each test below exercises a real-world PG construct and asserts
//! that none of the resolver-driven "this thing doesn't exist"
//! diagnostics fire. We're intentionally lenient: the LSP can't
//! statically resolve every PG construct, and crying wolf hurts
//! more than the missed real bug. Negative checks (where the same
//! pattern with a truly bogus column should still fire) live next
//! to the positives.
//!
//! The catalog includes one PK on `t.a` so the `ON CONFLICT (a)`
//! idiom doesn't trip `sql190` (unrelated rule about index match).

use dsl_analysis::{Diagnostic, run};
use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Constraint, ConstraintKind, Schema, Table, TableKind};
use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve_with_source;

fn col(name: &str, data_type: &str) -> Column {
  Column {
    name: name.into(),
    data_type: data_type.into(),
    nullable: false,
    default: None,
    comment: None,
    generated: None,
    json_keys: None,
  }
}

fn tbl(name: &str, columns: Vec<Column>, constraints: Vec<Constraint>) -> Table {
  Table {
    schema: "public".into(),
    name: name.into(),
    kind: TableKind::Table,
    columns,
    constraints,
    indexes: vec![],
    triggers: vec![],
    policies: vec![],
    comment: None,
    row_estimate: None,
    owner: None,
  }
}

fn pk(name: &str, cols: &[&str]) -> Constraint {
  Constraint {
    name: name.into(),
    kind: ConstraintKind::PrimaryKey,
    columns: cols.iter().map(|s| (*s).into()).collect(),
    references: None,
    definition: None,
    inline: false,
  }
}

fn cat_idioms() -> Catalog {
  let users = tbl(
    "users",
    vec![col("id", "uuid"), col("email", "text"), col("name", "text")],
    vec![pk("pk_users_id", &["id"])],
  );
  let other = tbl("other", vec![col("id", "uuid"), col("col", "text"), col("v", "text")], vec![]);
  let t = tbl(
    "t",
    vec![
      col("id", "int"),
      col("a", "int"),
      col("b", "int"),
      col("col", "text"),
      col("jsonb_col", "jsonb"),
      col("v", "text"),
      col("x", "int"),
      col("y", "int"),
    ],
    vec![pk("pk_t_a", &["a"])],
  );
  let src = tbl("src", vec![col("id", "int"), col("v", "text")], vec![]);
  let events = tbl("events", vec![col("id", "int"), col("created_at", "timestamptz")], vec![]);
  let ids = tbl("ids", vec![col("id", "int")], vec![]);
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema { name: "public".into(), tables: vec![users, other, t, src, events, ids] }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

fn diags(src: &str) -> Vec<Diagnostic> {
  let c = cat_idioms();
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &c)
}

/// Returns the resolver-family codes (column/table/function/unknown)
/// fired for `src`. Any diagnostic outside this set is considered
/// out-of-scope for the idiom checks below -- this is what we
/// actually care about not crying wolf on.
fn resolver_fam(d: &[Diagnostic]) -> Vec<String> {
  const WATCH: &[&str] = &["sql001", "sql002", "sql003", "sql042", "sql348", "sql349", "sql350", "sql351"];
  d.iter().filter(|x| WATCH.contains(&x.code)).map(|x| format!("{}: {}", x.code, x.message)).collect()
}

// =============================================================
// 1. INSERT ... ON CONFLICT ... DO UPDATE SET col = EXCLUDED.col
// =============================================================

#[test]
fn idiom_excluded_column_resolves() {
  // `EXCLUDED.<col>` is PG's implicit alias for the row being
  // inserted; columns mirror the target table.
  let d = diags("INSERT INTO t (a, b) VALUES (1, 2) ON CONFLICT (a) DO UPDATE SET b = EXCLUDED.b;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "EXCLUDED.b must resolve cleanly: {hits:?}");
}

#[test]
fn idiom_excluded_with_returning() {
  let d = diags("INSERT INTO t (a, b) VALUES (1, 2) ON CONFLICT (a) DO UPDATE SET b = EXCLUDED.b RETURNING id;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "EXCLUDED + RETURNING must resolve cleanly: {hits:?}");
}

// =============================================================
// 2. UPDATE t SET col = other.col FROM other WHERE t.id = other.id
// =============================================================

#[test]
fn idiom_update_from_qualified() {
  let d = diags("UPDATE t SET col = other.col FROM other WHERE t.id = other.id;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "UPDATE FROM with qualified refs must resolve cleanly: {hits:?}");
}

#[test]
fn idiom_update_from_alias() {
  let d = diags("UPDATE t SET col = o.col FROM other o WHERE t.id = o.id;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "UPDATE FROM with aliased FROM table must resolve cleanly: {hits:?}");
}

// =============================================================
// 3. DELETE FROM t USING other WHERE t.id = other.id
// =============================================================

#[test]
fn idiom_delete_using_qualified() {
  let d = diags("DELETE FROM t USING other WHERE t.id = other.id;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "DELETE USING with qualified refs must resolve cleanly: {hits:?}");
}

#[test]
fn idiom_delete_using_alias() {
  let d = diags("DELETE FROM t USING other o WHERE t.id = o.id;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "DELETE USING with aliased table must resolve cleanly: {hits:?}");
}

// =============================================================
// 4. WITH RECURSIVE r(n) AS (... self-reference ...)
// =============================================================

#[test]
fn idiom_recursive_cte_self_reference() {
  // The recursive arm refers to `r` -- both the CTE name (as a
  // FROM source) and `r.n` (as a column). Neither may be flagged.
  let d = diags("WITH RECURSIVE r(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM r WHERE n < 10) SELECT * FROM r;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "Recursive CTE self-ref must resolve cleanly: {hits:?}");
}

#[test]
fn idiom_recursive_cte_outer_select_uses_column() {
  let d = diags("WITH RECURSIVE r(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM r WHERE n < 10) SELECT n FROM r;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "Recursive CTE column ref in outer SELECT must resolve: {hits:?}");
}

// =============================================================
// 5. SELECT (jsonb_col->>'key')::int -- jsonb operator parse
// =============================================================

#[test]
fn idiom_jsonb_text_arrow() {
  // `jsonb_col->>'key'` -- the literal `'key'` is an operator
  // argument, not a column-named-key. Must NOT fire sql002 on
  // `key` (the literal is not a column ref).
  let d = diags("SELECT (jsonb_col->>'key')::int FROM t;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "jsonb ->> operator must not flag literal as unknown column: {hits:?}");
}

#[test]
fn idiom_jsonb_arrow() {
  let d = diags("SELECT jsonb_col->'key' FROM t;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "jsonb -> operator must resolve cleanly: {hits:?}");
}

#[test]
fn idiom_jsonb_path_extract() {
  let d = diags("SELECT jsonb_col #>> '{a,b}' FROM t;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "jsonb #>> path operator must resolve cleanly: {hits:?}");
}

// =============================================================
// 6. SELECT array_agg(x ORDER BY y) -- ORDER BY inside aggregate args
// =============================================================

#[test]
fn idiom_array_agg_order_by_inner() {
  // `ORDER BY y` inside the aggregate's arg list -- `y` is a
  // genuine column ref and must resolve.
  let d = diags("SELECT array_agg(x ORDER BY y) FROM t;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "aggregate inner ORDER BY column must resolve: {hits:?}");
}

#[test]
fn idiom_string_agg_order_by_inner() {
  let d = diags("SELECT string_agg(col, ',' ORDER BY id) FROM t;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "string_agg inner ORDER BY must resolve cleanly: {hits:?}");
}

// =============================================================
// 7. SELECT * FROM generate_series(1, 10) AS s(n)
// =============================================================

#[test]
fn idiom_generate_series_with_column_alias() {
  // `generate_series(1,10) AS s(n)` -- `s.n` is a column from the
  // explicit alias list and must resolve.
  let d = diags("SELECT * FROM generate_series(1, 10) AS s(n) WHERE s.n > 5;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "function-call FROM + column alias must resolve: {hits:?}");
}

#[test]
fn idiom_generate_series_no_alias() {
  // `SELECT * FROM generate_series(1,10);` -- no alias at all.
  // The resolver-family codes must stay quiet.
  let d = diags("SELECT * FROM generate_series(1, 10);");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "function-call FROM without alias must not trip resolver: {hits:?}");
}

// =============================================================
// 8. SELECT * FROM (VALUES (1, 'a'), ...) AS v(id, name)
// =============================================================

#[test]
fn idiom_values_subquery_with_aliases() {
  let d = diags("SELECT * FROM (VALUES (1, 'a'), (2, 'b')) AS v(id, name) WHERE v.id = 1;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "VALUES subquery with column aliases must resolve: {hits:?}");
}

// =============================================================
// 9. ANY / ALL with array literal and subquery
// =============================================================

#[test]
fn idiom_any_array_literal() {
  let d = diags("SELECT * FROM t WHERE id = ANY(ARRAY[1,2,3]);");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "ANY(ARRAY[...]) must resolve cleanly: {hits:?}");
}

#[test]
fn idiom_any_scalar_subquery() {
  let d = diags("SELECT * FROM t WHERE id = ANY(SELECT id FROM ids);");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "ANY(subquery) must resolve cleanly: {hits:?}");
}

#[test]
fn idiom_all_array_literal() {
  let d = diags("SELECT * FROM t WHERE id > ALL(ARRAY[1,2,3]);");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "ALL(ARRAY[...]) must resolve cleanly: {hits:?}");
}

// =============================================================
// 10. System-schema reads -- pg_catalog + information_schema
// =============================================================

#[test]
fn idiom_pg_catalog_read() {
  // We don't introspect pg_catalog; any column ref against
  // pg_catalog.* tables must be accepted leniently.
  let d = diags("SELECT * FROM pg_catalog.pg_class WHERE relkind = 'r';");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "pg_catalog read must not trip resolver: {hits:?}");
}

#[test]
fn idiom_information_schema_read() {
  let d = diags("SELECT table_schema FROM information_schema.tables;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "information_schema read must not trip resolver: {hits:?}");
}

#[test]
fn idiom_pg_catalog_with_join() {
  let d = diags("SELECT c.relname FROM pg_catalog.pg_class c WHERE c.relkind = 'r';");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "pg_catalog with alias + join must not trip resolver: {hits:?}");
}

// =============================================================
// 11. MERGE INTO ... USING ... WHEN MATCHED THEN UPDATE SET ...
// =============================================================

#[test]
fn idiom_merge_into() {
  // PG15+ MERGE -- the parser may not fully model this, but
  // resolver-family rules MUST stay quiet rather than false-fire.
  let d = diags("MERGE INTO t USING src ON t.id = src.id WHEN MATCHED THEN UPDATE SET v = src.v;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "MERGE statement must not trip resolver-family rules: {hits:?}");
}

// =============================================================
// 12. SELECT EXTRACT(YEAR FROM created_at)
// =============================================================

#[test]
fn idiom_extract_year_from_column() {
  // `EXTRACT(YEAR FROM created_at)` -- the keyword `YEAR` is the
  // first arg; `created_at` is the actual column reference.
  let d = diags("SELECT EXTRACT(YEAR FROM created_at) FROM events;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "EXTRACT with keyword first arg must resolve column: {hits:?}");
}

#[test]
fn idiom_extract_epoch_from_column() {
  let d = diags("SELECT EXTRACT(EPOCH FROM created_at) FROM events;");
  let hits = resolver_fam(&d);
  assert!(hits.is_empty(), "EXTRACT(EPOCH FROM col) must resolve cleanly: {hits:?}");
}

// =============================================================
// Negative sanity checks: real typos must still fire.
// =============================================================

#[test]
fn idiom_negative_unknown_column_still_fires() {
  let d = diags("SELECT bogus_nope_xyz FROM users;");
  assert!(
    d.iter().any(|x| x.code == "sql002"),
    "real unknown column must still fire sql002: {d:?}"
  );
}

#[test]
fn idiom_negative_unknown_table_still_fires() {
  let d = diags("SELECT * FROM nonexistent_table_xyz;");
  assert!(
    d.iter().any(|x| x.code == "sql001"),
    "real unresolved table must still fire sql001: {d:?}"
  );
}
