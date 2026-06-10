//! MySQL / MariaDB schema introspection.
//!
//! Queries `information_schema` for tables, columns, key constraints,
//! indexes, and triggers. MySQL groups objects by schema (== database),
//! so we exclude the built-in admin schemas (`mysql`, `information_schema`,
//! `performance_schema`, `sys`) and surface everything else.

use crate::driver::DriverError;
use crate::spec::ConnectionSpec;
use crate::util::strip_outer_parens;
use dsl_catalog::{
  CATALOG_VERSION, Catalog, Column, Constraint, ConstraintKind, ConstraintRef, Function, FunctionArg, IndexDef, Schema,
  Table, TableKind, Trigger,
};
use sqlx::{MySqlPool, Row};
use std::collections::BTreeMap;

const ADMIN_SCHEMAS: &str = "'mysql','information_schema','performance_schema','sys'";

pub async fn run(pool: &MySqlPool, spec: &ConnectionSpec) -> Result<Catalog, DriverError> {
  let mut schemas: BTreeMap<String, Schema> = BTreeMap::new();

  // Schemas (databases).
  let schemas_sql = format!(
    "SELECT schema_name FROM information_schema.schemata \
         WHERE schema_name NOT IN ({ADMIN_SCHEMAS}) ORDER BY schema_name"
  );
  for row in sqlx::query(&schemas_sql).fetch_all(pool).await.map_err(io_err)? {
    let name: String = row.try_get("SCHEMA_NAME").or_else(|_| row.try_get(0)).map_err(io_err)?;
    schemas.entry(name.clone()).or_insert_with(|| Schema { name, tables: Vec::new() });
  }

  // Tables + views. For base tables we also pull the storage engine,
  // collation, AUTO_INCREMENT seed, and row format so hover can show the
  // trailing `ENGINE=... DEFAULT CHARSET=...` option clause.
  let tables_sql = format!(
    "SELECT table_schema, table_name, table_type, engine, table_collation, auto_increment, row_format, table_comment \
         FROM information_schema.tables \
         WHERE table_schema NOT IN ({ADMIN_SCHEMAS}) \
         ORDER BY table_schema, table_name"
  );
  let mut by_table: BTreeMap<(String, String), Table> = BTreeMap::new();
  for row in sqlx::query(&tables_sql).fetch_all(pool).await.map_err(io_err)? {
    let schema: String = row.try_get("TABLE_SCHEMA").or_else(|_| row.try_get(0)).map_err(io_err)?;
    let name: String = row.try_get("TABLE_NAME").or_else(|_| row.try_get(1)).map_err(io_err)?;
    let ttype: String = row.try_get("TABLE_TYPE").or_else(|_| row.try_get(2)).map_err(io_err)?;
    let kind = match ttype.as_str() {
      "VIEW" => TableKind::View,
      _ => TableKind::Table,
    };
    // Option metadata is NULL for views; only base tables carry it.
    let options = if matches!(kind, TableKind::Table) {
      let engine: Option<String> = row.try_get("ENGINE").ok();
      let collation: Option<String> = row.try_get("TABLE_COLLATION").ok();
      let auto_increment: Option<i64> = row.try_get("AUTO_INCREMENT").ok();
      let row_format: Option<String> = row.try_get("ROW_FORMAT").ok();
      render_table_options(engine.as_deref(), collation.as_deref(), auto_increment, row_format.as_deref())
    } else {
      None
    };
    // `table_comment` is the `COMMENT='...'` text. MySQL reports the literal
    // "VIEW" here for views (not a user comment), so drop that and empties.
    let comment: Option<String> = row.try_get("TABLE_COMMENT").ok().filter(|s: &String| !s.is_empty() && s != "VIEW");
    by_table.insert(
      (schema.clone(), name.clone()),
      Table {
        schema,
        name,
        kind,
        columns: Vec::new(),
        constraints: Vec::new(),
        indexes: Vec::new(),
        triggers: Vec::new(),
        policies: Vec::new(),
        comment,
        row_estimate: None,
        owner: None,
        definition: None,
        strict: false,
        options,
      },
    );
  }

  // View definitions. `information_schema.views.view_definition` is the
  // SELECT body MySQL stores for the view; attach it to the matching entry.
  let views_sql = format!(
    "SELECT table_schema, table_name, view_definition \
         FROM information_schema.views \
         WHERE table_schema NOT IN ({ADMIN_SCHEMAS})"
  );
  if let Ok(rows) = sqlx::query(&views_sql).fetch_all(pool).await {
    for row in rows {
      let schema: String = match row.try_get("TABLE_SCHEMA").or_else(|_| row.try_get(0)) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let name: String = match row.try_get("TABLE_NAME").or_else(|_| row.try_get(1)) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let def: Option<String> = row.try_get("VIEW_DEFINITION").ok().filter(|s: &String| !s.is_empty());
      if let Some(t) = by_table.get_mut(&(schema, name)) {
        t.definition = def;
      }
    }
  }

  // Columns. `generation_expression` is empty for plain columns and carries
  // the (parenthesised, backtick-quoted) expression for STORED/VIRTUAL
  // generated columns; `extra` flags `auto_increment` and the generated kind.
  let cols_sql = format!(
    "SELECT table_schema, table_name, column_name, column_type, is_nullable, column_default, column_comment, \
                generation_expression, extra \
         FROM information_schema.columns \
         WHERE table_schema NOT IN ({ADMIN_SCHEMAS}) \
         ORDER BY table_schema, table_name, ordinal_position"
  );
  for row in sqlx::query(&cols_sql).fetch_all(pool).await.map_err(io_err)? {
    let schema: String = row.try_get("TABLE_SCHEMA").or_else(|_| row.try_get(0)).map_err(io_err)?;
    let table: String = row.try_get("TABLE_NAME").or_else(|_| row.try_get(1)).map_err(io_err)?;
    let name: String = row.try_get("COLUMN_NAME").or_else(|_| row.try_get(2)).map_err(io_err)?;
    let data_type: String = row.try_get("COLUMN_TYPE").or_else(|_| row.try_get(3)).map_err(io_err)?;
    let nullable_str: String = row.try_get("IS_NULLABLE").or_else(|_| row.try_get(4)).map_err(io_err)?;
    let mut default: Option<String> = row.try_get("COLUMN_DEFAULT").ok();
    let comment: Option<String> = row.try_get("COLUMN_COMMENT").ok().filter(|s: &String| !s.is_empty());
    let gen_expr: Option<String> = row.try_get("GENERATION_EXPRESSION").ok();
    let extra: String = row.try_get("EXTRA").unwrap_or_default();
    let generated = gen_expr.filter(|e| !e.is_empty()).map(|e| strip_outer_parens(&e).to_string());
    // AUTO_INCREMENT columns have no `column_default` row but behave like
    // one for lint/completion purposes (omitting them in INSERT is fine),
    // mirroring how PG SERIAL columns carry a `nextval(...)` default.
    if default.is_none() && extra.to_ascii_lowercase().contains("auto_increment") {
      default = Some("AUTO_INCREMENT".to_string());
    }
    if let Some(t) = by_table.get_mut(&(schema, table)) {
      t.columns.push(Column {
        name,
        data_type,
        nullable: nullable_str.eq_ignore_ascii_case("YES"),
        default,
        comment,
        generated,
        json_keys: None,
      });
    }
  }

  // Indexes (one row per (table, index, column); aggregate).
  let idx_sql = format!(
    "SELECT table_schema, table_name, index_name, column_name, non_unique, seq_in_index \
         FROM information_schema.statistics \
         WHERE table_schema NOT IN ({ADMIN_SCHEMAS}) \
         ORDER BY table_schema, table_name, index_name, seq_in_index"
  );
  let mut idx_acc: BTreeMap<(String, String, String), (bool, Vec<String>)> = BTreeMap::new();
  for row in sqlx::query(&idx_sql).fetch_all(pool).await.map_err(io_err)? {
    let schema: String = row.try_get("TABLE_SCHEMA").or_else(|_| row.try_get(0)).map_err(io_err)?;
    let table: String = row.try_get("TABLE_NAME").or_else(|_| row.try_get(1)).map_err(io_err)?;
    let iname: String = row.try_get("INDEX_NAME").or_else(|_| row.try_get(2)).map_err(io_err)?;
    let col: String = row.try_get("COLUMN_NAME").or_else(|_| row.try_get(3)).map_err(io_err)?;
    let non_unique: i64 = row.try_get("NON_UNIQUE").or_else(|_| row.try_get(4)).map_err(io_err)?;
    let entry = idx_acc.entry((schema, table, iname)).or_insert((non_unique == 0, Vec::new()));
    entry.1.push(col);
  }
  for ((schema, table, iname), (unique, cols)) in idx_acc {
    if let Some(t) = by_table.get_mut(&(schema, table)) {
      t.indexes.push(IndexDef { name: iname, columns: cols, unique, definition: None });
    }
  }

  // Key constraints (PRIMARY / UNIQUE / FOREIGN).
  let kc_sql = format!(
    "SELECT tc.table_schema, tc.table_name, tc.constraint_name, tc.constraint_type, \
                kcu.column_name, kcu.referenced_table_schema, kcu.referenced_table_name, kcu.referenced_column_name \
         FROM information_schema.table_constraints tc \
         LEFT JOIN information_schema.key_column_usage kcu \
           ON kcu.table_schema = tc.table_schema \
          AND kcu.table_name = tc.table_name \
          AND kcu.constraint_name = tc.constraint_name \
         WHERE tc.table_schema NOT IN ({ADMIN_SCHEMAS}) \
         ORDER BY tc.table_schema, tc.table_name, tc.constraint_name, kcu.ordinal_position"
  );
  #[allow(clippy::type_complexity)]
  let mut con_acc: BTreeMap<(String, String, String), (ConstraintKind, Vec<String>, Option<ConstraintRef>)> =
    BTreeMap::new();
  for row in sqlx::query(&kc_sql).fetch_all(pool).await.map_err(io_err)? {
    let schema: String = row.try_get("TABLE_SCHEMA").or_else(|_| row.try_get(0)).map_err(io_err)?;
    let table: String = row.try_get("TABLE_NAME").or_else(|_| row.try_get(1)).map_err(io_err)?;
    let cname: String = row.try_get("CONSTRAINT_NAME").or_else(|_| row.try_get(2)).map_err(io_err)?;
    let ctype: String = row.try_get("CONSTRAINT_TYPE").or_else(|_| row.try_get(3)).map_err(io_err)?;
    let col: Option<String> = row.try_get("COLUMN_NAME").ok();
    let ref_schema: Option<String> = row.try_get("REFERENCED_TABLE_SCHEMA").ok();
    let ref_table: Option<String> = row.try_get("REFERENCED_TABLE_NAME").ok();
    let ref_col: Option<String> = row.try_get("REFERENCED_COLUMN_NAME").ok();
    let kind = match ctype.as_str() {
      "PRIMARY KEY" => ConstraintKind::PrimaryKey,
      "UNIQUE" => ConstraintKind::Unique,
      "FOREIGN KEY" => ConstraintKind::ForeignKey,
      _ => continue, // CHECK constraints come from information_schema.check_constraints below
    };
    let entry = con_acc.entry((schema, table, cname)).or_insert((kind, Vec::new(), None));
    if let Some(c) = col {
      entry.1.push(c);
    }
    if let (Some(rs), Some(rt), Some(rc)) = (ref_schema, ref_table, ref_col) {
      let r = entry.2.get_or_insert(ConstraintRef { schema: rs, table: rt, columns: Vec::new() });
      r.columns.push(rc);
    }
  }
  for ((schema, table, cname), (kind, columns, references)) in con_acc {
    if let Some(t) = by_table.get_mut(&(schema, table)) {
      t.constraints.push(Constraint { name: cname, kind, columns, references, definition: None, inline: false });
    }
  }

  // CHECK constraints (MySQL 8.0.16+ / MariaDB 10.2+). The view does not
  // exist on older servers, so a failed query degrades to no CHECKs rather
  // than aborting the whole introspection. `check_constraints` carries the
  // clause text but not the table on MySQL, so we recover the table via a
  // join to `table_constraints`.
  let chk_sql = format!(
    "SELECT cc.constraint_schema, tc.table_name, cc.constraint_name, cc.check_clause \
         FROM information_schema.check_constraints cc \
         JOIN information_schema.table_constraints tc \
           ON tc.constraint_schema = cc.constraint_schema \
          AND tc.constraint_name = cc.constraint_name \
          AND tc.constraint_type = 'CHECK' \
         WHERE cc.constraint_schema NOT IN ({ADMIN_SCHEMAS})"
  );
  if let Ok(rows) = sqlx::query(&chk_sql).fetch_all(pool).await {
    for row in rows {
      let schema: String = match row.try_get("CONSTRAINT_SCHEMA").or_else(|_| row.try_get(0)) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let table: String = match row.try_get("TABLE_NAME").or_else(|_| row.try_get(1)) {
        Ok(v) => v,
        Err(_) => continue,
      };
      let cname: String = row.try_get("CONSTRAINT_NAME").or_else(|_| row.try_get(2)).unwrap_or_default();
      let clause: Option<String> = row.try_get("CHECK_CLAUSE").ok();
      if let Some(t) = by_table.get_mut(&(schema, table)) {
        t.constraints.push(Constraint {
          name: cname,
          kind: ConstraintKind::Check,
          columns: Vec::new(),
          references: None,
          definition: clause,
          inline: false,
        });
      }
    }
  }

  // Triggers.
  let trg_sql = format!(
    "SELECT event_object_schema, event_object_table, trigger_name, action_timing, event_manipulation, action_orientation, action_statement \
         FROM information_schema.triggers \
         WHERE event_object_schema NOT IN ({ADMIN_SCHEMAS})"
  );
  for row in sqlx::query(&trg_sql).fetch_all(pool).await.map_err(io_err)? {
    let schema: String = row.try_get("EVENT_OBJECT_SCHEMA").or_else(|_| row.try_get(0)).map_err(io_err)?;
    let table: String = row.try_get("EVENT_OBJECT_TABLE").or_else(|_| row.try_get(1)).map_err(io_err)?;
    let name: String = row.try_get("TRIGGER_NAME").or_else(|_| row.try_get(2)).map_err(io_err)?;
    let timing: String = row.try_get("ACTION_TIMING").or_else(|_| row.try_get(3)).map_err(io_err)?;
    let event: String = row.try_get("EVENT_MANIPULATION").or_else(|_| row.try_get(4)).map_err(io_err)?;
    let granularity: String = row.try_get("ACTION_ORIENTATION").or_else(|_| row.try_get(5)).map_err(io_err)?;
    if let Some(t) = by_table.get_mut(&(schema, table)) {
      t.triggers.push(Trigger {
        name,
        timing,
        event,
        granularity,
        function: String::new(), // MySQL triggers inline body, no function ref
      });
    }
  }

  // Fold tables back into their schema buckets.
  for ((schema_name, _), table) in by_table {
    let schema = schemas.entry(schema_name.clone()).or_insert_with(|| Schema { name: schema_name, tables: Vec::new() });
    schema.tables.push(table);
  }

  let functions = routines(pool).await?;

  Ok(Catalog {
    version: CATALOG_VERSION,
    connection_id: spec.name.clone(),
    schemas: schemas.into_values().collect(),
    functions,
    types: Vec::new(),
    roles: Vec::new(),
    sequences: Vec::new(),  // MySQL has no native sequences (AUTO_INCREMENT lives on columns).
    extensions: Vec::new(), // MySQL has no PG-style extensions.
  })
}

/// Stored functions and procedures from `information_schema.routines`,
/// with their parameter lists folded in from `information_schema.parameters`.
///
/// Best-effort: a user without `SELECT` on the metadata simply sees fewer
/// rows (MySQL scopes `information_schema` by privilege), and a hard error
/// degrades to an empty list rather than failing the whole introspection --
/// mirroring the Postgres path so a locked-down account still gets tables.
async fn routines(pool: &MySqlPool) -> Result<Vec<Function>, DriverError> {
  // Parameters first, keyed by (schema, routine name). For functions
  // MySQL emits an ordinal_position 0 row carrying the RETURNS type with
  // a NULL parameter_name -- skip it here; the return type comes from
  // `routines.dtd_identifier` below.
  let params_sql = format!(
    "SELECT specific_schema, specific_name, parameter_name, dtd_identifier \
         FROM information_schema.parameters \
         WHERE specific_schema NOT IN ({ADMIN_SCHEMAS}) AND ordinal_position > 0 \
         ORDER BY specific_schema, specific_name, ordinal_position"
  );
  let mut params: BTreeMap<(String, String), Vec<FunctionArg>> = BTreeMap::new();
  match sqlx::query(&params_sql).fetch_all(pool).await {
    Ok(rows) => {
      for row in rows {
        let schema: String = row.try_get("SPECIFIC_SCHEMA").or_else(|_| row.try_get(0)).map_err(io_err)?;
        let routine: String = row.try_get("SPECIFIC_NAME").or_else(|_| row.try_get(1)).map_err(io_err)?;
        let name: Option<String> = row.try_get("PARAMETER_NAME").ok();
        let data_type: String = row.try_get("DTD_IDENTIFIER").or_else(|_| row.try_get(3)).unwrap_or_default();
        params.entry((schema, routine)).or_default().push(FunctionArg { name, data_type });
      }
    }
    Err(_) => return Ok(Vec::new()),
  }

  let routines_sql = format!(
    "SELECT routine_schema, routine_name, routine_type, dtd_identifier, routine_comment, routine_definition \
         FROM information_schema.routines \
         WHERE routine_schema NOT IN ({ADMIN_SCHEMAS}) \
         ORDER BY routine_schema, routine_name"
  );
  let rows = match sqlx::query(&routines_sql).fetch_all(pool).await {
    Ok(rows) => rows,
    Err(_) => return Ok(Vec::new()),
  };

  let mut functions = Vec::with_capacity(rows.len());
  for row in rows {
    let schema: String = row.try_get("ROUTINE_SCHEMA").or_else(|_| row.try_get(0)).map_err(io_err)?;
    let name: String = row.try_get("ROUTINE_NAME").or_else(|_| row.try_get(1)).map_err(io_err)?;
    let rtype: String = row.try_get("ROUTINE_TYPE").or_else(|_| row.try_get(2)).unwrap_or_default();
    // dtd_identifier is the RETURNS type for FUNCTIONs and NULL for
    // PROCEDUREs; report procedures as returning `void` so hover/completion
    // can tell the two apart.
    let return_type: Option<String> = row.try_get("DTD_IDENTIFIER").ok();
    let return_type = match return_type {
      Some(t) if !t.is_empty() => t,
      _ if rtype.eq_ignore_ascii_case("PROCEDURE") => "void".to_string(),
      _ => String::new(),
    };
    let arguments = params.remove(&(schema.clone(), name.clone())).unwrap_or_default();
    // Surface the body in hover the same way the Postgres path does: stash a
    // reconstructed `CREATE FUNCTION/PROCEDURE ... <body>` in `comment` (the
    // renderer treats a `CREATE`-prefixed comment as the function source).
    // `routine_definition` is the body block MySQL stores; the signature is
    // rebuilt from the parameters we already gathered. Fall back to the
    // routine's COMMENT text (a docstring) when no body is available.
    let routine_def: Option<String> = row.try_get("ROUTINE_DEFINITION").ok().filter(|s: &String| !s.is_empty());
    let comment = match routine_def {
      Some(body) => Some(render_routine_source(&schema, &name, &rtype, &arguments, &return_type, &body)),
      None => row.try_get("ROUTINE_COMMENT").ok().filter(|s: &String| !s.is_empty()),
    };
    functions.push(Function { schema, name, arguments, return_type, comment });
  }

  Ok(functions)
}

/// Rebuild a `CREATE FUNCTION` / `CREATE PROCEDURE` statement from the routine
/// metadata so hover can show the source. MySQL's `routine_definition` holds
/// only the body block, so the signature is reconstructed from the parameters.
fn render_routine_source(
  schema: &str,
  name: &str,
  rtype: &str,
  args: &[FunctionArg],
  return_type: &str,
  body: &str,
) -> String {
  let is_procedure = rtype.eq_ignore_ascii_case("PROCEDURE");
  let arglist = args
    .iter()
    .map(|a| match &a.name {
      Some(n) => format!("{n} {}", a.data_type),
      None => a.data_type.clone(),
    })
    .collect::<Vec<_>>()
    .join(", ");
  let kw = if is_procedure { "CREATE PROCEDURE" } else { "CREATE FUNCTION" };
  let mut s = format!("{kw} {schema}.{name}({arglist})");
  if !is_procedure && !return_type.is_empty() && return_type != "void" {
    s.push_str(&format!(" RETURNS {return_type}"));
  }
  s.push('\n');
  s.push_str(body);
  s
}

/// Build the trailing table-option clause MySQL shows in `SHOW CREATE TABLE`
/// from `information_schema.tables` columns: storage engine, an AUTO_INCREMENT
/// seed (only when meaningfully > 1), the default charset + collation, and a
/// non-default row format. Returns `None` when nothing notable is set.
fn render_table_options(
  engine: Option<&str>,
  collation: Option<&str>,
  auto_increment: Option<i64>,
  row_format: Option<&str>,
) -> Option<String> {
  let mut parts: Vec<String> = Vec::new();
  if let Some(e) = engine.filter(|e| !e.is_empty()) {
    parts.push(format!("ENGINE={e}"));
  }
  if let Some(n) = auto_increment.filter(|&n| n > 1) {
    parts.push(format!("AUTO_INCREMENT={n}"));
  }
  if let Some(c) = collation.filter(|c| !c.is_empty()) {
    // Charset is the collation's leading segment (utf8mb4_0900_ai_ci -> utf8mb4).
    let charset = c.split('_').next().unwrap_or(c);
    parts.push(format!("DEFAULT CHARSET={charset}"));
    parts.push(format!("COLLATE={c}"));
  }
  // Dynamic is the modern InnoDB default -- only surface deliberate formats.
  if let Some(rf) = row_format.filter(|r| !r.is_empty() && !r.eq_ignore_ascii_case("dynamic")) {
    parts.push(format!("ROW_FORMAT={}", rf.to_ascii_uppercase()));
  }
  if parts.is_empty() { None } else { Some(parts.join(" ")) }
}

fn io_err(e: sqlx::Error) -> DriverError {
  DriverError::Introspect(e.to_string())
}

#[cfg(test)]
mod tests {
  use super::render_routine_source;
  use dsl_catalog::FunctionArg;

  fn arg(name: &str, ty: &str) -> FunctionArg {
    FunctionArg { name: Some(name.into()), data_type: ty.into() }
  }

  #[test]
  fn function_source_has_signature_and_returns() {
    let args = [arg("a", "int"), arg("b", "int")];
    let src = render_routine_source("app", "add", "FUNCTION", &args, "int", "RETURN a + b");
    assert_eq!(src, "CREATE FUNCTION app.add(a int, b int) RETURNS int\nRETURN a + b");
    // Must start with CREATE so the hover renderer treats it as source.
    assert!(src.to_ascii_uppercase().starts_with("CREATE"));
  }

  #[test]
  fn procedure_source_omits_returns() {
    let args = [arg("id", "int")];
    let src = render_routine_source("app", "touch", "PROCEDURE", &args, "void", "BEGIN UPDATE t SET seen = 1 WHERE id = id; END");
    assert!(src.starts_with("CREATE PROCEDURE app.touch(id int)\n"), "{src}");
    assert!(!src.contains("RETURNS"), "{src}");
  }

  use super::render_table_options;

  #[test]
  fn table_options_full_clause() {
    let opts = render_table_options(Some("InnoDB"), Some("utf8mb4_0900_ai_ci"), Some(1000), Some("Compressed"));
    assert_eq!(
      opts.as_deref(),
      Some("ENGINE=InnoDB AUTO_INCREMENT=1000 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci ROW_FORMAT=COMPRESSED")
    );
  }

  #[test]
  fn table_options_drops_noise() {
    // AUTO_INCREMENT=1 and a Dynamic row format are defaults -> omitted.
    let opts = render_table_options(Some("InnoDB"), Some("utf8mb4_general_ci"), Some(1), Some("Dynamic"));
    assert_eq!(opts.as_deref(), Some("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci"));
  }

  #[test]
  fn table_options_empty_when_nothing_set() {
    assert_eq!(render_table_options(None, None, None, None), None);
    assert_eq!(render_table_options(Some(""), Some(""), Some(1), Some("")), None);
  }
}
