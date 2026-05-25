//! MySQL / MariaDB schema introspection.
//!
//! Queries `information_schema` for tables, columns, key constraints,
//! indexes, and triggers. MySQL groups objects by schema (== database),
//! so we exclude the built-in admin schemas (`mysql`, `information_schema`,
//! `performance_schema`, `sys`) and surface everything else.

use crate::driver::DriverError;
use crate::spec::ConnectionSpec;
use dsl_catalog::{
  CATALOG_VERSION, Catalog, Column, Constraint, ConstraintKind, ConstraintRef, IndexDef, Schema, Table, TableKind,
  Trigger,
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

  // Tables + views.
  let tables_sql = format!(
    "SELECT table_schema, table_name, table_type \
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
        comment: None,
      },
    );
  }

  // Columns.
  let cols_sql = format!(
    "SELECT table_schema, table_name, column_name, column_type, is_nullable, column_default, column_comment \
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
    let default: Option<String> = row.try_get("COLUMN_DEFAULT").ok();
    let comment: Option<String> = row.try_get("COLUMN_COMMENT").ok().filter(|s: &String| !s.is_empty());
    if let Some(t) = by_table.get_mut(&(schema, table)) {
      t.columns.push(Column { name, data_type, nullable: nullable_str.eq_ignore_ascii_case("YES"), default, comment, generated: None, json_keys: None });
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
      _ => continue, // CHECK constraints handled via different schema view in newer MySQL
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
      t.constraints.push(Constraint { name: cname, kind, columns, references, definition: None });
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

  Ok(Catalog {
    version: CATALOG_VERSION,
    connection_id: spec.name.clone(),
    schemas: schemas.into_values().collect(),
    functions: Vec::new(), // TODO: information_schema.routines
    types: Vec::new(),
    roles: Vec::new(),
    sequences: Vec::new(),  // MySQL has no native sequences (AUTO_INCREMENT lives on columns).
    extensions: Vec::new(), // MySQL has no PG-style extensions.
  })
}

fn io_err(e: sqlx::Error) -> DriverError {
  DriverError::Introspect(e.to_string())
}
