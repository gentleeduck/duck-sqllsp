//! Data shape of the schema catalog. JSON-serialisable for caching.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
  pub version: u32,
  pub connection_id: String,
  pub schemas: Vec<Schema>,
  pub functions: Vec<Function>,
  /// User-defined types (enum / domain / composite). Populated by PG
  /// introspection from `pg_type`, `information_schema.domains`, and
  /// `pg_class` (composites). Default empty so older on-disk catalog
  /// snapshots continue to deserialise.
  #[serde(default)]
  pub types: Vec<Type>,
  /// Role names from `pg_roles` -- consumed by sql169
  /// owner_to_unknown_role and by completion / hover of GRANT TO /
  /// OWNER TO. Default empty so cached catalogs remain forward-
  /// compatible.
  #[serde(default)]
  pub roles: Vec<String>,
  /// Sequences from `pg_sequences`. Hover / completion / nextval()
  /// argument validation use these. Default empty so older snapshots
  /// stay forward-compatible.
  #[serde(default)]
  pub sequences: Vec<Sequence>,
  /// Installed extensions from `pg_extension`. Lets hover annotate
  /// `CREATE EXTENSION` and lets sql checks for known extensions
  /// (pgcrypto, uuid-ossp, postgis...) know what is available.
  #[serde(default)]
  pub extensions: Vec<Extension>,
}

/// Sequence object. Mirrors the columns of `pg_sequences`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sequence {
  pub schema: String,
  pub name: String,
  /// `bigint` / `integer` / `smallint`.
  pub data_type: String,
  pub start_value: i64,
  pub min_value: i64,
  pub max_value: i64,
  pub increment_by: i64,
  pub cycle: bool,
  /// True when this sequence is owned by a column (created
  /// implicitly by SERIAL / BIGSERIAL / GENERATED AS IDENTITY).
  pub owned_by_column: Option<String>,
  #[serde(default)]
  pub comment: Option<String>,
}

/// Installed Postgres extension. Mirrors `pg_extension` joined with
/// `pg_namespace` for the install schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
  pub name: String,
  pub schema: String,
  pub version: String,
  #[serde(default)]
  pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
  pub name: String,
  pub tables: Vec<Table>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
  pub schema: String,
  pub name: String,
  pub kind: TableKind,
  pub columns: Vec<Column>,
  pub constraints: Vec<Constraint>,
  pub indexes: Vec<IndexDef>,
  #[serde(default)]
  pub triggers: Vec<Trigger>,
  #[serde(default)]
  pub policies: Vec<Policy>,
  #[serde(default)]
  pub comment: Option<String>,
  /// Live planner row estimate (`pg_class.reltuples`). None when not
  /// fetched (offline mode, MySQL/SQLite drivers, brand-new table that
  /// hasn't been ANALYZEd). Used by code-lens row-count hint and the
  /// BRIN-on-small-table diagnostic.
  #[serde(default)]
  pub row_estimate: Option<f64>,
  /// Object owner role. Populated by live introspection (`pg_class.relowner`)
  /// or by source-text scan of `ALTER TABLE ... OWNER TO <role>` /
  /// `CREATE TABLE ... OWNER <role>`. Used by the hover renderer to
  /// show ownership at-a-glance.
  #[serde(default)]
  pub owner: Option<String>,
  /// For views / materialized views: the defining query (the `SELECT`
  /// body, without the `CREATE VIEW ... AS` prefix or trailing `;`).
  /// Populated by live introspection (`pg_get_viewdef` / MySQL
  /// `view_definition` / SQLite `sqlite_master.sql`) and by the offline
  /// `CREATE VIEW` source scan. `None` for ordinary tables. Used by hover
  /// to show what a view actually selects. Default `None` so older
  /// catalog snapshots stay forward-compatible.
  #[serde(default)]
  pub definition: Option<String>,
  /// SQLite `CREATE TABLE ... STRICT` -- the per-column declared types are
  /// rigidly enforced. Orthogonal to [`TableKind::WithoutRowid`] (a table
  /// may be both `STRICT` and `WITHOUT ROWID`), so it's a flag rather than a
  /// table kind. Always `false` for other dialects. Default `false` so older
  /// catalog snapshots stay forward-compatible.
  #[serde(default)]
  pub strict: bool,
  /// Dialect-specific trailing table-option clause, rendered as DDL text and
  /// appended after the column list by hover -- e.g. MySQL
  /// `ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci`.
  /// `None` when there are no extra options (or for dialects that have none).
  /// Default `None` so older catalog snapshots stay forward-compatible.
  #[serde(default)]
  pub options: Option<String>,
}

/// Row-level security policy attached to a table. Mirrors `pg_policies`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
  pub name: String,
  /// PERMISSIVE / RESTRICTIVE.
  pub permissive: String,
  /// Comma-separated role list, or `PUBLIC`.
  pub roles: String,
  /// ALL / SELECT / INSERT / UPDATE / DELETE.
  pub command: String,
  /// USING expression text.
  pub using_expr: Option<String>,
  /// WITH CHECK expression text.
  pub check_expr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
  pub name: String,
  /// BEFORE / AFTER / INSTEAD OF
  pub timing: String,
  /// INSERT / UPDATE / DELETE / TRUNCATE -- space-joined when multiple.
  pub event: String,
  /// ROW or STATEMENT
  pub granularity: String,
  /// schema.function_name
  pub function: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableKind {
  Table,
  View,
  MaterializedView,
  /// SQLite `CREATE TABLE ... WITHOUT ROWID`. Behaves like an ordinary
  /// table for DML, but has no implicit `rowid` and requires an explicit
  /// PRIMARY KEY -- surfaced so hover / completion can flag the
  /// optimisation. Other dialects never produce this variant.
  WithoutRowid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
  pub name: String,
  pub data_type: String,
  pub nullable: bool,
  #[serde(default)]
  pub default: Option<String>,
  #[serde(default)]
  pub comment: Option<String>,
  /// GENERATED ALWAYS AS (...) STORED expression. Stored as the
  /// parenthesised expression text without the enclosing keywords --
  /// hover renders the full form. None for non-generated columns.
  #[serde(default)]
  pub generated: Option<String>,
  /// Known top-level JSON keys for a `json` / `jsonb` column. Used by
  /// the JSON-path completion provider so typing
  /// `jsonb_path_query(col, '$.|` can suggest known keys. Populated
  /// from `-- @json-keys: a, b, c` annotation comments above the
  /// column declaration or from a future runtime sample.
  #[serde(default)]
  pub json_keys: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
  pub name: String,
  pub kind: ConstraintKind,
  pub columns: Vec<String>,
  #[serde(default)]
  pub references: Option<ConstraintRef>,
  /// `pg_get_constraintdef(con.oid)` -- the full DDL fragment. Holds the
  /// CHECK body, the FK action clauses, etc. Used by hover; never None
  /// for entries fetched from Postgres but Option-typed so older
  /// catalog snapshots still deserialise.
  #[serde(default)]
  pub definition: Option<String>,
  /// True when this constraint was declared inline on a column
  /// (`id int PRIMARY KEY` or `... REFERENCES other(id)`). The hover
  /// renderer folds inline constraints back onto the column row instead
  /// of emitting a separate `CONSTRAINT ...` line, which mirrors how the
  /// user wrote the table. `false` for table-level constraints and for
  /// anything coming back from live PG introspection (those are always
  /// rendered as top-level).
  #[serde(default)]
  pub inline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintKind {
  PrimaryKey,
  ForeignKey,
  Unique,
  Check,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintRef {
  pub schema: String,
  pub table: String,
  pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDef {
  pub name: String,
  pub columns: Vec<String>,
  pub unique: bool,
  /// `pg_get_indexdef(oid)` -- the CREATE INDEX text. Used by hover to
  /// show the full definition (column list, method, partial WHERE).
  #[serde(default)]
  pub definition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
  pub schema: String,
  pub name: String,
  pub arguments: Vec<FunctionArg>,
  pub return_type: String,
  #[serde(default)]
  pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionArg {
  pub name: Option<String>,
  pub data_type: String,
}

/// User-defined type. Mirrors the three CREATE TYPE flavours Postgres
/// supports (enum, domain, composite) plus a future-proof `Other` for
/// range / shell / base types we don't model yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Type {
  pub schema: String,
  pub name: String,
  pub kind: TypeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeKind {
  /// `CREATE TYPE x AS ENUM (...)`.
  Enum,
  /// `CREATE DOMAIN x AS base CHECK (...)`.
  Domain,
  /// `CREATE TYPE x AS (field1 type1, ...)`.
  Composite,
}
