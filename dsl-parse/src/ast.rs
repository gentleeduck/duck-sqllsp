//! The internal AST shape consumed by every downstream crate.
//!
//! Kept small on purpose: only the fields we need for completion, hover,
//! and analysis. Adding a field is cheap; removing one once a backend has
//! grown to fill it costs more, so we start conservative.

use serde::Serialize;
use text_size::TextRange;

#[derive(Debug, Clone, Serialize)]
pub struct Statement {
  pub range: TextRange,
  pub kind: StatementKind,
}

#[derive(Debug, Clone, Serialize)]
pub enum StatementKind {
  Select(SelectStmt),
  Insert(InsertStmt),
  Update(UpdateStmt),
  Delete(DeleteStmt),
  CreateTable(CreateTableStmt),
  AlterTable(AlterTableStmt),
  DropTable(DropTableStmt),
  /// Any statement we don't model in v0.1, or one that failed to parse.
  Unknown {
    text: String,
  },
}

// ---------------------------------------------------------------------------
// SELECT
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
pub struct SelectStmt {
  pub projections: Vec<Projection>,
  pub from: Vec<TableRef>,
  pub joins: Vec<JoinClause>,
  pub where_clause: Option<Expr>,
  /// CTE table names declared in a leading `WITH x AS (...) [, y AS (...)]`
  /// clause. Populated by the parser backend so the resolver can bind
  /// them as scope tables -- otherwise referencing the CTE in the outer
  /// SELECT looks like an unresolved table to completion + diagnostics.
  #[serde(default)]
  pub cte_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum Projection {
  Star,
  QualifiedStar(String),
  Expr { expr: Expr, alias: Option<String> },
}

// ---------------------------------------------------------------------------
// Tables and joins
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
pub struct TableRef {
  /// `schema.table` -- schema is the part before the dot when present.
  pub schema: Option<String>,
  pub name: String,
  pub alias: Option<String>,
  pub range: TextRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct JoinClause {
  pub kind: JoinKind,
  pub table: TableRef,
  pub on: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JoinKind {
  Inner,
  Left,
  Right,
  Full,
  Cross,
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub enum Expr {
  Column {
    qualifier: Option<String>,
    name: String,
    range: TextRange,
  },
  Literal(String),
  BinaryOp {
    op: String,
    left: Box<Expr>,
    right: Box<Expr>,
  },
  Call {
    name: String,
    args: Vec<Expr>,
  },
  /// Anything we don't model individually yet -- stringified upstream AST.
  Other(String),
  /// Flat container of sub-expressions. Used when the parser can't yet
  /// build a structured AST for a clause (WHERE / ON predicates,
  /// argument lists in unsupported constructs) but still wants
  /// downstream rules to see the column references inside it. Walkers
  /// recurse element-wise.
  List(Vec<Expr>),
}

// ---------------------------------------------------------------------------
// DML
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
pub struct InsertStmt {
  pub table: TableRef,
  pub columns: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateStmt {
  pub table: TableRef,
  pub assignments: Vec<(String, Expr)>,
  pub where_clause: Option<Expr>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DeleteStmt {
  pub table: TableRef,
  pub where_clause: Option<Expr>,
}

// ---------------------------------------------------------------------------
// DDL
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateTableStmt {
  pub table: TableRef,
  pub if_not_exists: bool,
  pub columns: Vec<ColumnDef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnDef {
  pub name: String,
  pub type_name: String,
  pub nullable: bool,
  pub default: Option<String>,
  pub range: TextRange,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AlterTableStmt {
  pub table: TableRef,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DropTableStmt {
  pub tables: Vec<TableRef>,
  pub if_exists: bool,
}
