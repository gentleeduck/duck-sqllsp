//! Postgres schema introspection.
//!
//! Runs three queries against `information_schema` / `pg_catalog`:
//!   1. schemas (excluding pg_catalog, information_schema, pg_toast)
//!   2. tables and views, with their schema
//!   3. columns, with data_type, nullability, default
//!
//! Results are folded into the [`Catalog`] shape so downstream code in
//! `dsl-completion` / `dsl-hover` / `dsl-analysis` reads the same struct
//! whether the data was loaded from cache or fetched live.

use crate::driver::DriverError;
use crate::spec::ConnectionSpec;
use crate::util::strip_outer_parens;
use dsl_catalog::{
  CATALOG_VERSION, Catalog, Column, Constraint, ConstraintKind, ConstraintRef, Extension, Function, FunctionArg,
  IndexDef, Policy, Schema, Sequence, Table, TableKind, Trigger, Type, TypeKind,
};
use sqlx::PgPool;
use std::collections::BTreeMap;

const SCHEMAS_SQL: &str = "
SELECT schema_name
FROM information_schema.schemata
WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
ORDER BY schema_name
";

const TABLES_SQL: &str = "
SELECT table_schema, table_name, table_type
FROM information_schema.tables
WHERE table_schema NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
ORDER BY table_schema, table_name
";

// View and materialized-view defining queries. `pg_get_viewdef(oid, true)`
// returns the pretty-printed SELECT body (no `CREATE VIEW ... AS` prefix).
const VIEWDEFS_SQL: &str = "
SELECT n.nspname AS schema, c.relname AS name, pg_catalog.pg_get_viewdef(c.oid, true) AS def
FROM pg_catalog.pg_class c
JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('v', 'm')
  AND n.nspname NOT IN ('pg_catalog', 'information_schema')
ORDER BY n.nspname, c.relname
";

const COLUMNS_SQL: &str = "
SELECT table_schema, table_name, column_name, data_type,
       is_nullable::text AS is_nullable,
       column_default,
       generation_expression
FROM information_schema.columns
WHERE table_schema NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
ORDER BY table_schema, table_name, ordinal_position
";

// Table constraints (PK / FK / UNIQUE / CHECK) keyed on pg_constraint so
// we avoid information_schema's noisy system-generated NOT NULL rows
// (e.g. `2200_16906_4_not_null`). One row per constraint -- column lists
// are produced by expanding `conkey` against `pg_attribute`.
//
// `check_expr` is the textual body of CHECK constraints; FK info is the
// referenced (schema, table, columns).
const CONSTRAINTS_SQL: &str = "
SELECT
    n.nspname   AS schema,
    c.relname   AS \"table\",
    con.conname AS name,
    con.contype::text AS kind,
    (
        SELECT string_agg(a.attname, ',' ORDER BY ord.ord)
        FROM unnest(con.conkey) WITH ORDINALITY AS ord(attnum, ord)
        JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = ord.attnum
    ) AS columns,
    fn.nspname  AS ref_schema,
    fc.relname  AS ref_table,
    (
        SELECT string_agg(a.attname, ',' ORDER BY ord.ord)
        FROM unnest(con.confkey) WITH ORDINALITY AS ord(attnum, ord)
        JOIN pg_attribute a ON a.attrelid = fc.oid AND a.attnum = ord.attnum
    ) AS ref_columns,
    pg_get_constraintdef(con.oid) AS check_expr
FROM pg_constraint con
JOIN pg_class     c  ON c.oid = con.conrelid
JOIN pg_namespace n  ON n.oid = c.relnamespace
LEFT JOIN pg_class     fc ON fc.oid = con.confrelid
LEFT JOIN pg_namespace fn ON fn.oid = fc.relnamespace
WHERE con.contype IN ('p','f','u','c')
  AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
ORDER BY n.nspname, c.relname, con.conname
";

// Every index on a user table -- including the implicit ones attached
// to PRIMARY KEY / UNIQUE constraints. The hover renderer shows them
// all so users see what btree the planner actually has. One row per
// (index, column); the per-index column list is assembled in Rust.
const INDEXES_SQL: &str = "
SELECT
    n.nspname AS schema,
    c.relname AS table,
    i.relname AS index_name,
    a.attname AS column_name,
    ix.indisunique AS is_unique,
    pg_get_indexdef(i.oid) AS definition
FROM pg_index ix
JOIN pg_class i  ON i.oid = ix.indexrelid
JOIN pg_class c  ON c.oid = ix.indrelid
JOIN pg_namespace n ON n.oid = c.relnamespace
JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = ANY(ix.indkey)
WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
ORDER BY n.nspname, c.relname, i.relname, a.attnum
";

// Triggers attached to user tables. The event manipulation column can
// list multiple verbs (INSERT, UPDATE, DELETE, TRUNCATE) when one
// trigger handles several; information_schema gives them as separate
// rows. We collapse by trigger name in Rust.
const TRIGGERS_SQL: &str = "
SELECT
    event_object_schema AS schema,
    event_object_table  AS table,
    trigger_name        AS name,
    action_timing       AS timing,
    string_agg(event_manipulation, ' OR ') AS event,
    action_orientation  AS granularity,
    action_statement    AS function
FROM information_schema.triggers
WHERE trigger_schema NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
GROUP BY event_object_schema, event_object_table, trigger_name,
         action_timing, action_orientation, action_statement
ORDER BY event_object_schema, event_object_table, trigger_name
";

// Row-level security policies on user tables. Postgres exposes the
// already-parsed expressions through pg_policies as readable text.
const POLICIES_SQL: &str = "
SELECT
    schemaname  AS schema,
    tablename   AS \"table\",
    policyname  AS name,
    permissive  AS permissive,
    coalesce(array_to_string(roles, ', '), 'PUBLIC') AS roles,
    cmd         AS command,
    qual        AS using_expr,
    with_check  AS check_expr
FROM pg_policies
ORDER BY schemaname, tablename, policyname
";

// User-defined types -- ENUM / DOMAIN / COMPOSITE. Filters out the
// implicit per-relation row types Postgres creates for every table (those
// are reachable as `tablename%ROWTYPE` already and would otherwise
// double-count every table). Filters out array types (typcategory='A')
// for the same reason. System schemas are excluded.
const TYPES_SQL: &str = "
SELECT
    n.nspname  AS schema,
    t.typname  AS name,
    t.typtype::text AS kind
FROM pg_catalog.pg_type t
JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace
WHERE n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
  AND t.typtype IN ('e', 'd', 'c')
  AND t.typcategory <> 'A'
  AND NOT EXISTS (
      SELECT 1 FROM pg_catalog.pg_class c
      WHERE c.oid = t.typrelid AND c.relkind <> 'c'
  )
ORDER BY n.nspname, t.typname
";

// User-defined functions. We fetch the signature, return type, and the
// pg_get_functiondef() output (full CREATE OR REPLACE FUNCTION ... text).
// Excludes the system schemas and aggregate / window machinery so the
// catalog stays focused on functions a user wrote.
const FUNCTIONS_SQL: &str = "
SELECT
    n.nspname                                                AS schema,
    p.proname                                                AS name,
    pg_catalog.pg_get_function_identity_arguments(p.oid)     AS args,
    pg_catalog.pg_get_function_result(p.oid)                 AS result,
    pg_catalog.pg_get_functiondef(p.oid)                     AS def
FROM pg_catalog.pg_proc p
JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
  AND p.prokind IN ('f', 'p')  -- functions and procedures, not aggregates / window
ORDER BY n.nspname, p.proname
";

pub async fn run(pool: &PgPool, spec: &ConnectionSpec) -> Result<Catalog, DriverError> {
  let map_err = |e: sqlx::Error| DriverError::Introspect(e.to_string());

  // Schemas
  let schema_rows: Vec<(String,)> = sqlx::query_as(SCHEMAS_SQL).fetch_all(pool).await.map_err(map_err)?;
  let mut schemas: BTreeMap<String, Schema> =
    schema_rows.into_iter().map(|(name,)| (name.clone(), Schema { name, tables: Vec::new() })).collect();

  // Tables
  let table_rows: Vec<(String, String, String)> = sqlx::query_as(TABLES_SQL).fetch_all(pool).await.map_err(map_err)?;
  // Row estimates. `pg_class.reltuples` is the planner's estimate but is
  // `-1` for tables that have never been ANALYZED and stays `0` for
  // tables under the autovacuum threshold (50 rows by default). To get
  // a useful chip on small / freshly-seeded tables, also pull
  // `pg_stat_user_tables.n_live_tup` (the stats collector's live row
  // count) and prefer it when `reltuples <= 0`. Falls back to None
  // silently on permission denied; never breaks introspect.
  let row_estimates: Vec<(String, String, f32, Option<i64>)> = sqlx::query_as(
    "SELECT n.nspname::text, c.relname::text, c.reltuples, s.n_live_tup \
     FROM pg_class c \
     JOIN pg_namespace n ON c.relnamespace = n.oid \
     LEFT JOIN pg_stat_user_tables s ON s.relid = c.oid \
     WHERE c.relkind IN ('r', 'p', 'm') \
       AND n.nspname NOT IN ('pg_catalog', 'information_schema')",
  )
  .fetch_all(pool)
  .await
  .unwrap_or_default();
  let estimate_lookup: BTreeMap<(String, String), f64> = row_estimates
    .into_iter()
    .map(|(s, n, reltuples, live)| {
      let chosen = if reltuples > 0.0 {
        reltuples as f64
      } else {
        live.map(|v| v as f64).unwrap_or(0.0)
      };
      ((s, n), chosen)
    })
    .collect();
  // Per-table owner name resolved through pg_authid. Silently empty on
  // permission errors so the rest of introspect still works.
  let owner_rows: Vec<(String, String, String)> = sqlx::query_as(
    "SELECT n.nspname::text, c.relname::text, COALESCE(a.rolname, '')::text \
     FROM pg_class c \
     JOIN pg_namespace n ON c.relnamespace = n.oid \
     LEFT JOIN pg_authid a ON c.relowner = a.oid \
     WHERE c.relkind IN ('r', 'p', 'm', 'v') \
       AND n.nspname NOT IN ('pg_catalog', 'information_schema')",
  )
  .fetch_all(pool)
  .await
  .unwrap_or_default();
  let owner_lookup: BTreeMap<(String, String), String> =
    owner_rows.into_iter().filter(|(_, _, o)| !o.is_empty()).map(|(s, n, o)| ((s, n), o)).collect();
  // (schema, name) -> table index inside Catalog.schemas[s].tables[i].
  let mut table_index: BTreeMap<(String, String), (String, usize)> = BTreeMap::new();
  for (schema_name, table_name, table_type) in table_rows {
    let entry =
      schemas.entry(schema_name.clone()).or_insert_with(|| Schema { name: schema_name.clone(), tables: Vec::new() });
    let kind = match table_type.as_str() {
      "VIEW" => TableKind::View,
      "MATERIALIZED VIEW" => TableKind::MaterializedView,
      _ => TableKind::Table,
    };
    let idx = entry.tables.len();
    entry.tables.push(Table {
      schema: schema_name.clone(),
      name: table_name.clone(),
      kind,
      columns: Vec::new(),
      constraints: Vec::new(),
      indexes: Vec::new(),
      triggers: Vec::new(),
      policies: Vec::new(),
      comment: None,
      row_estimate: estimate_lookup.get(&(schema_name.clone(), table_name.clone())).copied(),
      owner: owner_lookup.get(&(schema_name.clone(), table_name.clone())).cloned(),
      definition: None,
      strict: false, options: None,
    });
    table_index.insert((schema_name, table_name), (entry.name.clone(), idx));
  }

  // View / materialized-view definitions. `pg_get_viewdef(oid, true)` gives
  // the pretty-printed SELECT body. Best-effort -- empty on error.
  if let Ok(rows) = sqlx::query_as::<_, (String, String, String)>(VIEWDEFS_SQL).fetch_all(pool).await {
    for (schema_name, view_name, def) in rows {
      if let Some((_, idx)) = table_index.get(&(schema_name.clone(), view_name))
        && let Some(s) = schemas.get_mut(&schema_name)
        && let Some(t) = s.tables.get_mut(*idx)
      {
        let def = def.trim().trim_end_matches(';').trim().to_string();
        t.definition = if def.is_empty() { None } else { Some(def) };
      }
    }
  }

  // Columns
  #[allow(clippy::type_complexity)]
  let column_rows: Vec<(String, String, String, String, String, Option<String>, Option<String>)> =
    sqlx::query_as(COLUMNS_SQL).fetch_all(pool).await.map_err(map_err)?;
  for (schema_name, table_name, column_name, data_type, is_nullable, default, gen_expr) in column_rows {
    if let Some((_, idx)) = table_index.get(&(schema_name.clone(), table_name.clone()))
      && let Some(s) = schemas.get_mut(&schema_name)
      && let Some(t) = s.tables.get_mut(*idx)
    {
      // `generation_expression` is NULL for plain columns and the
      // pg_get_expr() text (already parenthesised) for STORED generated
      // columns. Strip the outer parens to match the bare-expression
      // convention the offline source scanner and hover renderer use.
      let generated = gen_expr.filter(|e| !e.is_empty()).map(|e| strip_outer_parens(&e).to_string());
      t.columns.push(Column {
        name: column_name,
        data_type,
        nullable: is_nullable == "YES",
        default,
        comment: None,
        generated,
        json_keys: None,
      });
    }
  }

  // Constraints: one row per constraint with comma-separated column /
  // ref-column lists. `kind` is the single-char `pg_constraint.contype`
  // value (`p`, `f`, `u`, `c`). CHECK constraints carry the body in
  // `check_expr` (we stash it on the Constraint's `name` if the
  // catalog ever wants it -- for now it's discarded except for the
  // hover via the constraint identifier render).
  let constraint_rows = sqlx::query_as::<
    _,
    (
      String,         // schema
      String,         // table
      String,         // name
      String,         // kind (p/f/u/c)
      Option<String>, // columns (csv)
      Option<String>, // ref_schema
      Option<String>, // ref_table
      Option<String>, // ref_columns (csv)
      Option<String>, // check_expr
    ),
  >(CONSTRAINTS_SQL)
  .fetch_all(pool)
  .await;
  if let Ok(rows) = constraint_rows {
    for (schema, table_name, name, kind_str, columns, ref_schema, ref_table, ref_columns, check_expr) in rows {
      let kind = match kind_str.as_str() {
        "p" => ConstraintKind::PrimaryKey,
        "f" => ConstraintKind::ForeignKey,
        "u" => ConstraintKind::Unique,
        "c" => ConstraintKind::Check,
        _ => continue,
      };
      let cols: Vec<String> = columns.map(|s| s.split(',').map(|p| p.trim().to_string()).collect()).unwrap_or_default();
      let ref_cols: Vec<String> =
        ref_columns.map(|s| s.split(',').map(|p| p.trim().to_string()).collect()).unwrap_or_default();
      let references = match (ref_schema, ref_table) {
        (Some(rs), Some(rt)) => Some(ConstraintRef { schema: rs, table: rt, columns: ref_cols }),
        _ => None,
      };
      if let Some((_, idx)) = table_index.get(&(schema.clone(), table_name.clone()))
        && let Some(s) = schemas.get_mut(&schema)
        && let Some(t) = s.tables.get_mut(*idx)
      {
        t.constraints.push(Constraint {
          name,
          kind,
          columns: cols,
          references,
          definition: check_expr,
          inline: false,
        });
      }
    }
  }

  // Indexes (excluding the implicit primary-key / unique indexes already
  // covered by the constraints query). pg_index gives us indkey for
  // column ordering; pg_get_indexdef would be nicer but harder to
  // partition into per-column rows.
  if let Ok(rows) =
    sqlx::query_as::<_, (String, String, String, String, bool, String)>(INDEXES_SQL).fetch_all(pool).await
  {
    #[allow(clippy::type_complexity)]
    let mut grouped: BTreeMap<(String, String, String), (Vec<String>, bool, String)> = BTreeMap::new();
    for (schema, table, idx_name, column, unique, definition) in rows {
      let entry = grouped.entry((schema, table, idx_name)).or_default();
      entry.0.push(column);
      entry.1 = unique;
      entry.2 = definition;
    }
    for ((schema, table_name, name), (cols, unique, definition)) in grouped {
      if let Some((_, idx)) = table_index.get(&(schema.clone(), table_name.clone()))
        && let Some(s) = schemas.get_mut(&schema)
        && let Some(t) = s.tables.get_mut(*idx)
      {
        t.indexes.push(IndexDef {
          name,
          columns: cols,
          unique,
          definition: if definition.is_empty() { None } else { Some(definition) },
        });
      }
    }
  }

  // Triggers.
  if let Ok(rows) =
    sqlx::query_as::<_, (String, String, String, String, String, String, String)>(TRIGGERS_SQL).fetch_all(pool).await
  {
    for (schema, table_name, name, timing, event, granularity, function) in rows {
      if let Some((_, idx)) = table_index.get(&(schema.clone(), table_name.clone()))
        && let Some(s) = schemas.get_mut(&schema)
        && let Some(t) = s.tables.get_mut(*idx)
      {
        t.triggers.push(Trigger { name, timing, event, granularity, function });
      }
    }
  }

  // Policies (best-effort). `pg_policies` requires no special perms.
  if let Ok(rows) =
    sqlx::query_as::<_, (String, String, String, bool, String, String, Option<String>, Option<String>)>(POLICIES_SQL)
      .fetch_all(pool)
      .await
  {
    for (schema, table_name, name, permissive, roles, command, using_expr, check_expr) in rows {
      if let Some((_, idx)) = table_index.get(&(schema.clone(), table_name.clone()))
        && let Some(s) = schemas.get_mut(&schema)
        && let Some(t) = s.tables.get_mut(*idx)
      {
        t.policies.push(Policy {
          name,
          permissive: if permissive { "PERMISSIVE".into() } else { "RESTRICTIVE".into() },
          roles,
          command,
          using_expr,
          check_expr,
        });
      }
    }
  }

  // Functions. Best-effort: if the user lacks privileges or the
  // server is older than 9.0 we just leave the list empty rather than
  // failing the whole introspection.
  let mut functions: Vec<Function> = Vec::new();
  if let Ok(rows) = sqlx::query_as::<_, (String, String, String, String, String)>(FUNCTIONS_SQL).fetch_all(pool).await {
    for (schema, name, args, result, def) in rows {
      let arguments = parse_args(&args);
      functions.push(Function {
        schema,
        name,
        arguments,
        return_type: result,
        // We stash the full DDL in `comment` so dsl-hover can
        // render it without changing the catalog schema. The
        // rendered output prefixes a "Source" heading, so users
        // can tell apart this from a docstring.
        comment: if def.is_empty() { None } else { Some(def) },
      });
    }
  }

  // Types (enum / domain / composite). Best-effort -- empty on error.
  let mut types: Vec<Type> = Vec::new();
  if let Ok(rows) = sqlx::query_as::<_, (String, String, String)>(TYPES_SQL).fetch_all(pool).await {
    for (schema, name, kind_char) in rows {
      let kind = match kind_char.as_str() {
        "e" => TypeKind::Enum,
        "d" => TypeKind::Domain,
        "c" => TypeKind::Composite,
        _ => continue,
      };
      types.push(Type { schema, name, kind });
    }
  }

  // Roles -- consumed by sql169 owner_to_unknown_role and by
  // completion / hover of GRANT TO / OWNER TO. Skip rolname starting
  // with `pg_` (built-in PG internal roles).
  let mut roles: Vec<String> = Vec::new();
  if let Ok(rows) =
    sqlx::query_as::<_, (String,)>("SELECT rolname FROM pg_roles ORDER BY rolname").fetch_all(pool).await
  {
    roles = rows.into_iter().map(|(n,)| n).collect();
  }

  // Sequences from `pg_sequences` (the documented user-visible view
  // over `pg_class` + `pg_sequence`). Skips system schemas just like
  // tables. `owned_by_column` is derived from `pg_depend` -- a
  // sequence implicitly created by SERIAL or GENERATED AS IDENTITY
  // hangs off the column via a `auto`-class dependency.
  let mut sequences: Vec<Sequence> = Vec::new();
  if let Ok(rows) =
    sqlx::query_as::<_, (String, String, String, i64, i64, i64, i64, bool, Option<String>)>(SEQUENCES_SQL)
      .fetch_all(pool)
      .await
  {
    for (schema, name, data_type, start, min, max, inc, cycle, owned) in rows {
      sequences.push(Sequence {
        schema,
        name,
        data_type,
        start_value: start,
        min_value: min,
        max_value: max,
        increment_by: inc,
        cycle,
        owned_by_column: owned,
        comment: None,
      });
    }
  }

  // Installed extensions from `pg_extension` joined to `pg_namespace`
  // for the install schema. `pg_catalog` extensions (the bundled
  // plpgsql) are not filtered -- some users care that it's there.
  let mut extensions: Vec<Extension> = Vec::new();
  if let Ok(rows) = sqlx::query_as::<_, (String, String, String, Option<String>)>(EXTENSIONS_SQL).fetch_all(pool).await
  {
    for (name, schema, version, comment) in rows {
      extensions.push(Extension { name, schema, version, comment });
    }
  }

  Ok(Catalog {
    version: CATALOG_VERSION,
    connection_id: spec.name.clone(),
    schemas: schemas.into_values().collect(),
    functions,
    types,
    roles,
    sequences,
    extensions,
  })
}

const SEQUENCES_SQL: &str = "
SELECT
    s.schemaname::text AS schema,
    s.sequencename::text AS name,
    s.data_type::text AS data_type,
    s.start_value::bigint AS start_value,
    s.min_value::bigint AS min_value,
    s.max_value::bigint AS max_value,
    s.increment_by::bigint AS increment_by,
    s.cycle AS cycle,
    (
        SELECT nsp.nspname || '.' || cls.relname || '.' || att.attname
        FROM pg_depend d
        JOIN pg_class cls ON cls.oid = d.refobjid
        JOIN pg_namespace nsp ON nsp.oid = cls.relnamespace
        JOIN pg_attribute att ON att.attrelid = d.refobjid AND att.attnum = d.refobjsubid
        JOIN pg_class seqcls ON seqcls.oid = d.objid
        JOIN pg_namespace seqnsp ON seqnsp.oid = seqcls.relnamespace
        WHERE d.classid = 'pg_class'::regclass
          AND d.refclassid = 'pg_class'::regclass
          AND d.deptype IN ('a','i')
          AND seqcls.relkind = 'S'
          AND seqcls.relname = s.sequencename
          AND seqnsp.nspname = s.schemaname
        LIMIT 1
    )::text AS owned_by_column
FROM pg_sequences s
WHERE s.schemaname NOT IN ('pg_catalog','information_schema','pg_toast')
ORDER BY s.schemaname, s.sequencename
";

const EXTENSIONS_SQL: &str = "
SELECT
    e.extname::text         AS name,
    n.nspname::text         AS schema,
    e.extversion::text      AS version,
    obj_description(e.oid, 'pg_extension')::text AS comment
FROM pg_extension e
JOIN pg_namespace n ON n.oid = e.extnamespace
ORDER BY e.extname
";

/// Parse the output of `pg_get_function_identity_arguments`. The string
/// looks like `name1 type1, name2 type2` (names optional). Splits on
/// top-level commas; nested parens (e.g. `numeric(10,2)`) are respected.
fn parse_args(s: &str) -> Vec<FunctionArg> {
  if s.trim().is_empty() {
    return Vec::new();
  }
  let mut out = Vec::new();
  let mut depth: i32 = 0;
  let mut start = 0usize;
  for (i, ch) in s.char_indices() {
    match ch {
      '(' => depth += 1,
      ')' => depth -= 1,
      ',' if depth == 0 => {
        push_arg(&s[start..i], &mut out);
        start = i + 1;
      },
      _ => {},
    }
  }
  push_arg(&s[start..], &mut out);
  out
}

fn push_arg(raw: &str, out: &mut Vec<FunctionArg>) {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return;
  }
  // "name type" -> (Some(name), type). Otherwise just (None, trimmed).
  if let Some((head, rest)) = trimmed.split_once(char::is_whitespace) {
    let head_upper = head.to_ascii_uppercase();
    // pg may prefix arg modes (IN / OUT / INOUT / VARIADIC). Treat
    // those as a no-op; the name is still the next token.
    if matches!(head_upper.as_str(), "IN" | "OUT" | "INOUT" | "VARIADIC") {
      if let Some((name, ty)) = rest.trim_start().split_once(char::is_whitespace) {
        out.push(FunctionArg { name: Some(name.into()), data_type: ty.trim().into() });
        return;
      }
      out.push(FunctionArg { name: None, data_type: rest.trim().into() });
      return;
    }
    // Type may itself contain spaces (e.g. "character varying").
    // Heuristic: if `head` looks like an identifier and `rest`
    // starts with a recognised data-type word, treat head as name.
    if head.chars().all(|c| c.is_alphanumeric() || c == '_') {
      out.push(FunctionArg { name: Some(head.into()), data_type: rest.trim().into() });
      return;
    }
  }
  out.push(FunctionArg { name: None, data_type: trimmed.into() });
}
