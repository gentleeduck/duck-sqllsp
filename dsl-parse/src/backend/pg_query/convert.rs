//! pg_query protobuf node -> internal AST conversion.
//!
//! Only the slots downstream crates actually use are populated:
//!
//! - CREATE TABLE -> [`StatementKind::CreateTable`] with columns
//! - CREATE INDEX -> not yet a dedicated variant; reported as Unknown
//! - SELECT -> [`StatementKind::Select`] with FROM tables + optional WHERE
//!   expression text
//! - INSERT/UPDATE/DELETE -> respective variants with target table +
//!   presence-of-WHERE bit
//! - ALTER TABLE / DROP TABLE -> respective variants
//! - Everything else -> `StatementKind::Unknown { text }` so the feature
//!   stack still sees the raw SQL.
//!
//! Goal: full PG18 syntax coverage at parse-time, even when our AST
//! doesn't model the new constructs.

use crate::ast::{
  AlterTableStmt, ColumnDef, CreateTableStmt, DeleteStmt, DropTableStmt, Expr, InsertStmt, JoinClause, JoinKind,
  Projection, SelectStmt, StatementKind, TableRef, UpdateStmt,
};
use pg_query::protobuf::node::Node as PgNode;
use pg_query::protobuf::{ColumnDef as PgCol, CreateStmt, RangeVar};
use text_size::TextRange;

pub fn statement(node: &PgNode, text: &str) -> StatementKind {
  match node {
    PgNode::CreateStmt(c) => create_table(c, text),
    PgNode::SelectStmt(s) => StatementKind::Select(select(s, text)),
    PgNode::InsertStmt(i) => StatementKind::Insert(insert(i, text)),
    PgNode::UpdateStmt(u) => StatementKind::Update(update(u, text)),
    PgNode::DeleteStmt(d) => StatementKind::Delete(delete(d, text)),
    PgNode::AlterTableStmt(a) => StatementKind::AlterTable(alter_table(a)),
    PgNode::DropStmt(d) => StatementKind::DropTable(drop_table(d)),
    _ => StatementKind::Unknown { text: text.to_string() },
  }
}

// ---------------------------------------------------------------------------
// CREATE TABLE
// ---------------------------------------------------------------------------

fn create_table(stmt: &CreateStmt, _text: &str) -> StatementKind {
  let table = stmt.relation.as_ref().map(rangevar_to_tableref).unwrap_or_default();
  let mut columns = Vec::new();
  for elt in &stmt.table_elts {
    if let Some(PgNode::ColumnDef(c)) = elt.node.as_ref() {
      columns.push(column_def(c));
    }
  }
  let if_not_exists = stmt.if_not_exists;
  StatementKind::CreateTable(CreateTableStmt { table, if_not_exists, columns })
}

fn column_def(c: &PgCol) -> ColumnDef {
  let name = c.colname.clone();
  let type_name = c
    .type_name
    .as_ref()
    .map(|t| {
      let base = t
        .names
        .iter()
        .filter_map(|n| n.node.as_ref())
        .filter_map(|n| match n {
          PgNode::String(s) => Some(s.sval.clone()),
          _ => None,
        })
        .collect::<Vec<_>>()
        .join(".");
      // Append `[]` per array dim so downstream rules (sql230 GIN on
      // scalar) see the array suffix and don't false-positive on text[].
      let arr = "[]".repeat(t.array_bounds.len());
      format!("{base}{arr}")
    })
    .unwrap_or_default();
  // pg_query exposes nullability through constraints. A NOT NULL
  // constraint sets `contype = ConstrNotnull`. Defaults live in
  // `ConstrDefault` and stringify as the raw text via cooked_expr.
  let mut nullable = true;
  let mut default: Option<String> = None;
  use pg_query::protobuf::ConstrType;
  for cons in &c.constraints {
    if let Some(PgNode::Constraint(con)) = cons.node.as_ref() {
      // Constraint type enum (see pg_query::protobuf::ConstrType).
      // PRIMARY KEY implies NOT NULL semantically; we propagate
      // that here so completion/hover reflect runtime behaviour.
      let ct = ConstrType::try_from(con.contype).unwrap_or(ConstrType::Undefined);
      match ct {
        ConstrType::ConstrNotnull | ConstrType::ConstrPrimary => nullable = false,
        ConstrType::ConstrDefault if !con.raw_expr_string().is_empty() => {
          default = Some(con.raw_expr_string());
        },
        _ => {},
      }
    }
  }
  ColumnDef { name, type_name, nullable, default, range: TextRange::default() }
}

// ---------------------------------------------------------------------------
// SELECT
// ---------------------------------------------------------------------------

fn select(s: &pg_query::protobuf::SelectStmt, _text: &str) -> SelectStmt {
  let mut from: Vec<TableRef> = Vec::new();
  let mut joins: Vec<JoinClause> = Vec::new();
  for f in &s.from_clause {
    match f.node.as_ref() {
      Some(PgNode::RangeVar(r)) => from.push(rangevar_to_tableref(r)),
      Some(PgNode::JoinExpr(j)) => walk_join(j, &mut from, &mut joins),
      // `FROM generate_series(1, 10) AS number` -- function-call FROM.
      // Sentinel schema `<func>` flags the binding as synthetic so
      // catalog-driven rules (sql001/sql002) skip the catalog lookup.
      Some(PgNode::RangeFunction(rf)) => {
        let alias = rf.alias.as_ref().map(|a| a.aliasname.clone()).filter(|n| !n.is_empty());
        if let Some(alias_str) = alias {
          from.push(TableRef {
            schema: Some("<func>".into()),
            name: alias_str.clone(),
            alias: Some(alias_str),
            range: TextRange::default(),
          });
        }
      },
      // `FROM (SELECT ...) AS sub` -- subquery alias. Sentinel `<subq>`.
      Some(PgNode::RangeSubselect(rs)) => {
        let alias = rs.alias.as_ref().map(|a| a.aliasname.clone()).filter(|n| !n.is_empty());
        if let Some(alias_str) = alias {
          from.push(TableRef {
            schema: Some("<subq>".into()),
            name: alias_str.clone(),
            alias: Some(alias_str),
            range: TextRange::default(),
          });
        }
      },
      _ => {},
    }
  }
  let projections: Vec<Projection> = s
    .target_list
    .iter()
    .filter_map(|n| n.node.as_ref())
    .filter_map(|n| match n {
      PgNode::ResTarget(t) => Some(target_to_projection(t)),
      _ => None,
    })
    .collect();
  let cte_names: Vec<String> = s
    .with_clause
    .as_ref()
    .map(|wc| {
      wc.ctes
        .iter()
        .filter_map(|n| n.node.as_ref())
        .filter_map(|n| match n {
          PgNode::CommonTableExpr(cte) => Some(cte.ctename.clone()),
          _ => None,
        })
        .collect()
    })
    .unwrap_or_default();
  SelectStmt {
    projections,
    from,
    joins,
    where_clause: s.where_clause.as_ref().map(|n| extract_column_refs_expr(n.node.as_ref())),
    cte_names,
  }
}

/// Walk a pg_query node, collecting every `ColumnRef` it contains as an
/// `Expr::Column`. Wraps the result in `Expr::List` so downstream
/// walkers (e.g. unknown_column rule) can recurse element-wise. When the
/// node contains no ColumnRefs we still return an empty List rather than
/// `Other` so the variant is uniform across WHERE / ON / argument
/// contexts.
fn extract_column_refs_expr(node: Option<&PgNode>) -> Expr {
  let mut out = Vec::new();
  if let Some(n) = node {
    collect_column_refs(n, &mut out);
  }
  Expr::List(out)
}

fn collect_column_refs(node: &PgNode, out: &mut Vec<Expr>) {
  use pg_query::protobuf::node::Node as N;
  match node {
    N::ColumnRef(c) => {
      // pg_query represents `a.b` as a `fields` Vec of String/A_Star nodes.
      let parts: Vec<String> = c
        .fields
        .iter()
        .filter_map(|f| f.node.as_ref())
        .filter_map(|f| match f {
          N::String(s) => Some(s.sval.clone()),
          _ => None,
        })
        .collect();
      let (qualifier, name) = match parts.len() {
        1 => (None, parts.into_iter().next().unwrap()),
        2 => {
          let mut it = parts.into_iter();
          let q = it.next().unwrap();
          let n = it.next().unwrap();
          (Some(q), n)
        },
        3 => {
          // schema.table.col
          let mut it = parts.into_iter();
          let schema = it.next().unwrap();
          let table = it.next().unwrap();
          let n = it.next().unwrap();
          (Some(format!("{schema}.{table}")), n)
        },
        _ => return,
      };
      if name.is_empty() {
        return;
      }
      out.push(Expr::Column { qualifier, name, range: TextRange::default() });
    },
    N::AExpr(a) => {
      if let Some(l) = a.lexpr.as_ref().and_then(|n| n.node.as_ref()) {
        collect_column_refs(l, out);
      }
      if let Some(r) = a.rexpr.as_ref().and_then(|n| n.node.as_ref()) {
        collect_column_refs(r, out);
      }
    },
    N::BoolExpr(b) => {
      for arg in &b.args {
        if let Some(n) = arg.node.as_ref() {
          collect_column_refs(n, out);
        }
      }
    },
    N::FuncCall(f) => {
      for arg in &f.args {
        if let Some(n) = arg.node.as_ref() {
          collect_column_refs(n, out);
        }
      }
    },
    N::List(l) => {
      for item in &l.items {
        if let Some(n) = item.node.as_ref() {
          collect_column_refs(n, out);
        }
      }
    },
    N::SubLink(s) => {
      if let Some(t) = s.testexpr.as_ref().and_then(|n| n.node.as_ref()) {
        collect_column_refs(t, out);
      }
      // Subselect body: walk its targetList + WHERE so unknown columns
      // referenced inside `IN (SELECT noexist FROM ...)` / `EXISTS
      // (SELECT noexist FROM ...)` still surface as sql002.
      if let Some(PgNode::SelectStmt(subsel)) = s.subselect.as_ref().and_then(|n| n.node.as_ref()) {
        for target in &subsel.target_list {
          if let Some(n) = target.node.as_ref() {
            collect_column_refs(n, out);
          }
        }
        if let Some(w) = subsel.where_clause.as_ref().and_then(|n| n.node.as_ref()) {
          collect_column_refs(w, out);
        }
      }
    },
    N::ResTarget(rt) => {
      if let Some(n) = rt.val.as_ref().and_then(|n| n.node.as_ref()) {
        collect_column_refs(n, out);
      }
    },
    N::NullTest(t) => {
      if let Some(n) = t.arg.as_ref().and_then(|n| n.node.as_ref()) {
        collect_column_refs(n, out);
      }
    },
    N::CaseExpr(c) => {
      for w in &c.args {
        if let Some(n) = w.node.as_ref() {
          collect_column_refs(n, out);
        }
      }
      if let Some(d) = c.defresult.as_ref().and_then(|n| n.node.as_ref()) {
        collect_column_refs(d, out);
      }
    },
    N::CaseWhen(c) => {
      if let Some(e) = c.expr.as_ref().and_then(|n| n.node.as_ref()) {
        collect_column_refs(e, out);
      }
      if let Some(r) = c.result.as_ref().and_then(|n| n.node.as_ref()) {
        collect_column_refs(r, out);
      }
    },
    _ => {},
  }
}

/// Recursively unfold a JoinExpr -- the left arg can itself be another
/// JoinExpr (chained joins), the right arg is typically a RangeVar.
fn walk_join(j: &pg_query::protobuf::JoinExpr, from: &mut Vec<TableRef>, joins: &mut Vec<JoinClause>) {
  match j.larg.as_ref().and_then(|n| n.node.as_ref()) {
    Some(PgNode::RangeVar(r)) if from.is_empty() => {
      from.push(rangevar_to_tableref(r));
    },
    Some(PgNode::JoinExpr(inner)) => walk_join(inner, from, joins),
    Some(PgNode::RangeFunction(rf)) => {
      let alias = rf.alias.as_ref().map(|a| a.aliasname.clone()).filter(|n| !n.is_empty());
      if let Some(alias_str) = alias
        && from.is_empty()
      {
        from.push(TableRef {
          schema: Some("<func>".into()),
          name: alias_str.clone(),
          alias: Some(alias_str),
          range: TextRange::default(),
        });
      }
    },
    Some(PgNode::RangeSubselect(rs)) => {
      let alias = rs.alias.as_ref().map(|a| a.aliasname.clone()).filter(|n| !n.is_empty());
      if let Some(alias_str) = alias
        && from.is_empty()
      {
        from.push(TableRef {
          schema: Some("<subq>".into()),
          name: alias_str.clone(),
          alias: Some(alias_str),
          range: TextRange::default(),
        });
      }
    },
    _ => {},
  }
  match j.rarg.as_ref().and_then(|n| n.node.as_ref()) {
    Some(PgNode::RangeVar(r)) => {
      joins.push(JoinClause {
        kind: join_kind(j.jointype),
        table: rangevar_to_tableref(r),
        on: j.quals.as_ref().map(|n| extract_column_refs_expr(n.node.as_ref())),
      });
    },
    Some(PgNode::RangeFunction(rf)) => {
      let alias = rf.alias.as_ref().map(|a| a.aliasname.clone()).filter(|n| !n.is_empty());
      if let Some(alias_str) = alias {
        joins.push(JoinClause {
          kind: join_kind(j.jointype),
          table: TableRef {
            schema: Some("<func>".into()),
            name: alias_str.clone(),
            alias: Some(alias_str),
            range: TextRange::default(),
          },
          on: j.quals.as_ref().map(|n| extract_column_refs_expr(n.node.as_ref())),
        });
      }
    },
    Some(PgNode::RangeSubselect(rs)) => {
      let alias = rs.alias.as_ref().map(|a| a.aliasname.clone()).filter(|n| !n.is_empty());
      if let Some(alias_str) = alias {
        joins.push(JoinClause {
          kind: join_kind(j.jointype),
          table: TableRef {
            schema: Some("<subq>".into()),
            name: alias_str.clone(),
            alias: Some(alias_str),
            range: TextRange::default(),
          },
          on: j.quals.as_ref().map(|n| extract_column_refs_expr(n.node.as_ref())),
        });
      }
    },
    _ => {},
  }
}

/// Map the pg_query JoinType enum to our internal JoinKind.
fn join_kind(jt: i32) -> JoinKind {
  use pg_query::protobuf::JoinType;
  match JoinType::try_from(jt).unwrap_or(JoinType::Undefined) {
    JoinType::JoinLeft => JoinKind::Left,
    JoinType::JoinRight => JoinKind::Right,
    JoinType::JoinFull => JoinKind::Full,
    JoinType::JoinAnti => JoinKind::Left, // anti as left for highlight purposes
    JoinType::JoinSemi => JoinKind::Inner,
    _ => JoinKind::Inner,
  }
}

/// Convert a `ResTarget` (one item in the SELECT target list) into our
/// `Projection`. Recognises `*`, qualified `t.*`, and a bare column
/// reference; everything else stays as `Other`.
fn target_to_projection(t: &pg_query::protobuf::ResTarget) -> Projection {
  let alias = if t.name.is_empty() { None } else { Some(t.name.clone()) };
  let val = match t.val.as_ref().and_then(|n| n.node.as_ref()) {
    Some(v) => v,
    None => return Projection::Expr { expr: Expr::List(Vec::new()), alias },
  };
  match val {
    PgNode::ColumnRef(cref) => column_ref_to_projection(cref, alias),
    // Function calls / CASE / arithmetic / boolean expressions in the
    // projection: don't drop the entire predicate to `Other("")`.
    // Walk and collect every column reference so downstream rules
    // (sql002 unknown column, sql003 ambiguous, ...) can still flag
    // typos inside `length(noexist)` / `CASE WHEN noexist THEN ...`.
    other => Projection::Expr { expr: extract_column_refs_expr(Some(other)), alias },
  }
}

/// `ColumnRef.fields` holds an ordered list of identifier / A_Star
/// nodes. `["*"]` -> Projection::Star, `["t", "*"]` -> QualifiedStar,
/// `["col"]` or `["t", "col"]` -> column expression.
fn column_ref_to_projection(cref: &pg_query::protobuf::ColumnRef, alias: Option<String>) -> Projection {
  let parts: Vec<String> = cref
    .fields
    .iter()
    .filter_map(|n| n.node.as_ref())
    .map(|n| match n {
      PgNode::String(s) => s.sval.clone(),
      PgNode::AStar(_) => "*".into(),
      _ => String::new(),
    })
    .collect();
  // pg_query attaches a byte offset on the ColumnRef. Span it across
  // the full reference text (`q.name` or `name`) so the diagnostic
  // can highlight exactly the offending token, not the whole stmt.
  let range = if cref.location >= 0 {
    let start = cref.location as u32;
    let len = parts.iter().map(|s| s.len() as u32).sum::<u32>() + parts.len().saturating_sub(1) as u32; // dots
    TextRange::new(start.into(), (start + len.max(1)).into())
  } else {
    TextRange::default()
  };
  match parts.as_slice() {
    [s] if s == "*" => Projection::Star,
    [q, s] if s == "*" => Projection::QualifiedStar(q.clone()),
    [name] => Projection::Expr { expr: Expr::Column { qualifier: None, name: name.clone(), range }, alias },
    [q, name] => {
      Projection::Expr { expr: Expr::Column { qualifier: Some(q.clone()), name: name.clone(), range }, alias }
    },
    _ => Projection::Expr { expr: Expr::Other(String::new()), alias },
  }
}

// ---------------------------------------------------------------------------
// DML
// ---------------------------------------------------------------------------

fn insert(i: &pg_query::protobuf::InsertStmt, _text: &str) -> InsertStmt {
  let table = i.relation.as_ref().map(rangevar_to_tableref).unwrap_or_default();
  let columns = i
    .cols
    .iter()
    .filter_map(|n| match n.node.as_ref()? {
      PgNode::ResTarget(t) => Some(t.name.clone()),
      _ => None,
    })
    .collect();
  InsertStmt { table, columns }
}

fn update(u: &pg_query::protobuf::UpdateStmt, _text: &str) -> UpdateStmt {
  let table = u.relation.as_ref().map(rangevar_to_tableref).unwrap_or_default();
  // SET <col> = <expr>, ... -- target_list items are ResTargets with
  // `name` = column and `val` = expression node. The internal AST
  // only needs the column name + a placeholder expr; the live
  // catalog supplies type info.
  let assignments = u
    .target_list
    .iter()
    .filter_map(|n| match n.node.as_ref()? {
      PgNode::ResTarget(t) if !t.name.is_empty() => Some((t.name.clone(), Expr::Other(String::new()))),
      _ => None,
    })
    .collect();
  // `UPDATE t SET ... FROM <list> WHERE ...` -- the FROM list adds
  // additional table bindings visible in SET RHS and WHERE.
  let from_tables = collect_dml_from_tables(&u.from_clause);
  UpdateStmt {
    table,
    assignments,
    where_clause: u.where_clause.as_ref().map(|n| extract_column_refs_expr(n.node.as_ref())),
    from_tables,
  }
}

fn delete(d: &pg_query::protobuf::DeleteStmt, _text: &str) -> DeleteStmt {
  let table = d.relation.as_ref().map(rangevar_to_tableref).unwrap_or_default();
  // `DELETE FROM t USING <list> WHERE ...` -- USING list adds
  // additional bindings visible in WHERE and RETURNING.
  let using_tables = collect_dml_from_tables(&d.using_clause);
  DeleteStmt {
    table,
    where_clause: d.where_clause.as_ref().map(|n| extract_column_refs_expr(n.node.as_ref())),
    using_tables,
  }
}

/// Extract simple RangeVar table bindings from a DML FROM/USING list.
/// Subqueries, joins, and function calls are skipped -- only bare table
/// references contribute alias bindings.
fn collect_dml_from_tables(list: &[pg_query::protobuf::Node]) -> Vec<TableRef> {
  let mut out = Vec::new();
  for n in list {
    if let Some(PgNode::RangeVar(r)) = n.node.as_ref() {
      out.push(rangevar_to_tableref(r));
    }
  }
  out
}

fn alter_table(a: &pg_query::protobuf::AlterTableStmt) -> AlterTableStmt {
  AlterTableStmt { table: a.relation.as_ref().map(rangevar_to_tableref).unwrap_or_default() }
}

/// pg_query lumps DROP TABLE / VIEW / INDEX into the same DropStmt with
/// a removeType discriminator. The `objects` list holds the targets --
/// each is a List of strings forming `[schema, name]` (or just `[name]`).
fn drop_table(d: &pg_query::protobuf::DropStmt) -> DropTableStmt {
  let mut tables = Vec::new();
  for obj in &d.objects {
    if let Some(PgNode::List(list)) = obj.node.as_ref() {
      let parts: Vec<String> = list
        .items
        .iter()
        .filter_map(|n| match n.node.as_ref()? {
          PgNode::String(s) => Some(s.sval.clone()),
          _ => None,
        })
        .collect();
      let (schema, name) = match parts.as_slice() {
        [s, n] => (Some(s.clone()), n.clone()),
        [n] => (None, n.clone()),
        _ => continue,
      };
      tables.push(TableRef { schema, name, alias: None, range: TextRange::default() });
    }
  }
  DropTableStmt { tables, if_exists: d.missing_ok }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn rangevar_to_tableref(r: &RangeVar) -> TableRef {
  let schema = if r.schemaname.is_empty() { None } else { Some(r.schemaname.clone()) };
  let alias = r.alias.as_ref().map(|a| a.aliasname.clone()).filter(|n| !n.is_empty());
  // pg_query stores the source byte offset for the relation token on
  // `r.location`. Compute the name-token range by spanning that
  // start through `relname.len()`. Schema-qualified refs (`s.t`) still
  // narrow to just the relname; good enough for IDE highlighting.
  let range = if r.location >= 0 {
    let start = r.location as u32;
    let len = r.relname.len() as u32;
    TextRange::new(start.into(), (start + len).into())
  } else {
    TextRange::default()
  };
  TableRef { schema, name: r.relname.clone(), alias, range }
}

// pg_query doesn't always populate the cooked/raw expr string in a
// stable way; provide a small helper that picks the best available
// representation. Returns an empty string when nothing usable is set.
trait ConstraintExprText {
  fn raw_expr_string(&self) -> String;
}
impl ConstraintExprText for pg_query::protobuf::Constraint {
  fn raw_expr_string(&self) -> String {
    if !self.cooked_expr.is_empty() {
      return self.cooked_expr.clone();
    }
    // raw_expr is a Node; pg_query exposes deparse_protobuf to
    // round-trip a Node back to SQL but it requires the full
    // ParseResult shape. For now we return empty -- the live
    // catalog provides the authoritative default during
    // introspection.
    String::new()
  }
}
