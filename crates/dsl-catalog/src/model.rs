//! Data shape of the schema catalog. JSON-serialisable for caching.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub version: u32,
    pub connection_id: String,
    pub schemas: Vec<Schema>,
    pub functions: Vec<Function>,
    /// User-defined types (enum / domain / composite). Populated by
    /// PG introspection from `pg_type` + `information_schema.domains`
    /// + `pg_class` (composites). Default empty so older on-disk
    /// catalog snapshots continue to deserialise.
    #[serde(default)]
    pub types: Vec<Type>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintKind {
    PrimaryKey, ForeignKey, Unique, Check,
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
