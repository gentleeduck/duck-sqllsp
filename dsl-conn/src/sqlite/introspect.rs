//! SQLite schema introspection.
//!
//! SQLite has no `information_schema` worth using. We scan `sqlite_master`
//! for objects then `PRAGMA table_info` / `index_list` / `foreign_key_list`
//! to fill columns, indexes, and FK constraints.

use crate::driver::DriverError;
use crate::spec::ConnectionSpec;
use dsl_catalog::{
    Catalog, Column, Constraint, ConstraintKind, ConstraintRef, IndexDef, Schema, Table,
    TableKind, Trigger, CATALOG_VERSION,
};
use sqlx::{Row, SqlitePool};

const SCHEMA_NAME: &str = "main";

pub async fn run(pool: &SqlitePool, spec: &ConnectionSpec) -> Result<Catalog, DriverError> {
    let mut tables: Vec<Table> = Vec::new();
    let mut triggers_by_table: std::collections::BTreeMap<String, Vec<Trigger>> = Default::default();

    // Collect objects.
    let objs = sqlx::query("SELECT type, name, tbl_name FROM sqlite_master WHERE type IN ('table','view','trigger') AND name NOT LIKE 'sqlite_%' ORDER BY type, name")
        .fetch_all(pool)
        .await
        .map_err(io_err)?;

    for row in &objs {
        let kind: String = row.try_get("type").map_err(io_err)?;
        let name: String = row.try_get("name").map_err(io_err)?;
        let tbl_name: String = row.try_get("tbl_name").map_err(io_err)?;
        match kind.as_str() {
            "table" | "view" => {
                let table_kind = if kind == "view" { TableKind::View } else { TableKind::Table };
                let columns = fetch_columns(pool, &name).await?;
                let indexes = fetch_indexes(pool, &name).await?;
                let constraints = fetch_fks(pool, &name).await?;
                tables.push(Table {
                    schema: SCHEMA_NAME.into(),
                    name,
                    kind: table_kind,
                    columns,
                    constraints,
                    indexes,
                    triggers: Vec::new(),
                    policies: Vec::new(),
                    comment: None,
                });
            }
            "trigger" => {
                // Trigger metadata in sqlite_master is sparse; we record name + parent.
                triggers_by_table.entry(tbl_name).or_default().push(Trigger {
                    name,
                    timing: String::new(),
                    event: String::new(),
                    granularity: String::new(),
                    function: String::new(),
                });
            }
            _ => {}
        }
    }

    // Attach triggers to their tables.
    for t in tables.iter_mut() {
        if let Some(trs) = triggers_by_table.remove(&t.name) {
            t.triggers = trs;
        }
    }

    let schema = Schema {
        name: SCHEMA_NAME.into(),
        tables,
    };

    Ok(Catalog {
        version: CATALOG_VERSION,
        connection_id: spec.name.clone(),
        schemas: vec![schema],
        functions: Vec::new(),
        types: Vec::new(),
        roles: Vec::new(),
        sequences: Vec::new(),  // SQLite uses sqlite_sequence rows, not first-class sequence objects.
        extensions: Vec::new(),
    })
}

async fn fetch_columns(pool: &SqlitePool, table: &str) -> Result<Vec<Column>, DriverError> {
    let q = format!("PRAGMA table_info({})", quote_ident(table));
    let rows = sqlx::query(&q).fetch_all(pool).await.map_err(io_err)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let name: String = row.try_get("name").map_err(io_err)?;
        let data_type: String = row.try_get("type").map_err(io_err)?;
        let notnull: i64 = row.try_get("notnull").map_err(io_err)?;
        let default: Option<String> = row.try_get("dflt_value").ok();
        out.push(Column {
            name,
            data_type,
            nullable: notnull == 0,
            default,
            comment: None,
        });
    }
    Ok(out)
}

async fn fetch_indexes(pool: &SqlitePool, table: &str) -> Result<Vec<IndexDef>, DriverError> {
    let q = format!("PRAGMA index_list({})", quote_ident(table));
    let list = sqlx::query(&q).fetch_all(pool).await.map_err(io_err)?;
    let mut out = Vec::with_capacity(list.len());
    for row in list {
        let iname: String = row.try_get("name").map_err(io_err)?;
        let unique: i64 = row.try_get("unique").map_err(io_err)?;
        let info_q = format!("PRAGMA index_info({})", quote_ident(&iname));
        let info = sqlx::query(&info_q).fetch_all(pool).await.map_err(io_err)?;
        let cols: Vec<String> = info
            .into_iter()
            .filter_map(|r| r.try_get::<String, _>("name").ok())
            .collect();
        out.push(IndexDef { name: iname, columns: cols, unique: unique != 0, definition: None });
    }
    Ok(out)
}

async fn fetch_fks(pool: &SqlitePool, table: &str) -> Result<Vec<Constraint>, DriverError> {
    let q = format!("PRAGMA foreign_key_list({})", quote_ident(table));
    let rows = sqlx::query(&q).fetch_all(pool).await.map_err(io_err)?;
    use std::collections::BTreeMap;
    let mut acc: BTreeMap<i64, (Vec<String>, String, Vec<String>)> = BTreeMap::new();
    for row in rows {
        let id: i64 = row.try_get("id").map_err(io_err)?;
        let from: String = row.try_get("from").map_err(io_err)?;
        let to: String = row.try_get("to").map_err(io_err)?;
        let ref_table: String = row.try_get("table").map_err(io_err)?;
        let entry = acc.entry(id).or_insert((Vec::new(), ref_table, Vec::new()));
        entry.0.push(from);
        entry.2.push(to);
    }
    Ok(acc
        .into_iter()
        .map(|(id, (cols, ref_table, ref_cols))| Constraint {
            name: format!("fk_{}_{}", table, id),
            kind: ConstraintKind::ForeignKey,
            columns: cols,
            references: Some(ConstraintRef {
                schema: SCHEMA_NAME.into(),
                table: ref_table,
                columns: ref_cols,
            }),
            definition: None,
        })
        .collect())
}

fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn io_err(e: sqlx::Error) -> DriverError {
    DriverError::Introspect(e.to_string())
}
