//! Convert sqlparser's AST shape into our internal AST.
//!
//! sqlparser is rich enough that we only project the slices we currently
//! use. Anything we don't model becomes [`StatementKind::Unknown`] /
//! [`Expr::Other`]. This is fine: completion + analysis treat Unknown as
//! "best effort" and degrade gracefully.

use crate::ast::*;
use sqlparser::ast as sp;
use text_size::TextRange;

pub fn statement(s: sp::Statement, raw: &str) -> StatementKind {
  use sp::Statement as S;
  match s {
    S::Query(q) => query(*q, raw),
    S::Insert(ins) => StatementKind::Insert(InsertStmt {
      table: object_name(&ins.table_name),
      columns: ins.columns.iter().map(|c| c.value.clone()).collect(),
    }),
    S::Update { table, assignments, selection, from, .. } => {
      let from_tables: Vec<_> = from.iter().map(|t| table_factor(&t.relation)).collect();
      StatementKind::Update(UpdateStmt {
        table: table_factor(&table.relation),
        assignments: assignments
          .into_iter()
          .map(|a| (a.target.to_string(), Expr::Other(a.value.to_string())))
          .collect(),
        where_clause: selection.map(|e| Expr::Other(e.to_string())),
        from_tables,
      })
    },
    S::Delete(d) => {
      let from_vec = match &d.from {
        sp::FromTable::WithFromKeyword(v) | sp::FromTable::WithoutKeyword(v) => v,
      };
      let table = from_vec.first().map(|t| table_factor(&t.relation)).unwrap_or_default();
      let using_tables = d.using.iter().flat_map(|v| v.iter().map(|t| table_factor(&t.relation))).collect();
      StatementKind::Delete(DeleteStmt {
        table,
        where_clause: d.selection.map(|e| Expr::Other(e.to_string())),
        using_tables,
      })
    },
    S::CreateTable(ct) => StatementKind::CreateTable(CreateTableStmt {
      table: object_name(&ct.name),
      if_not_exists: ct.if_not_exists,
      columns: ct.columns.iter().map(column_def).collect(),
    }),
    S::AlterTable { name, .. } => StatementKind::AlterTable(AlterTableStmt { table: object_name(&name) }),
    S::Drop { object_type: sp::ObjectType::Table, if_exists, names, .. } => {
      StatementKind::DropTable(DropTableStmt { if_exists, tables: names.iter().map(object_name).collect() })
    },
    other => StatementKind::Unknown { text: other.to_string() },
  }
}

fn query(q: sp::Query, raw: &str) -> StatementKind {
  let ctes: Vec<String> =
    q.with.as_ref().map(|w| w.cte_tables.iter().map(|c| c.alias.name.value.clone()).collect()).unwrap_or_default();
  if let sp::SetExpr::Select(sel) = *q.body {
    let mut stmt = select(*sel, raw);
    stmt.cte_names = ctes;
    return StatementKind::Select(stmt);
  }
  StatementKind::Unknown { text: q.to_string() }
}

fn select(s: sp::Select, raw: &str) -> SelectStmt {
  let projections = s.projection.iter().map(|p| projection(p, raw)).collect();
  let mut from = Vec::new();
  let mut joins = Vec::new();
  for twj in &s.from {
    from.push(table_factor(&twj.relation));
    for j in &twj.joins {
      joins.push(JoinClause {
        kind: join_kind(&j.join_operator),
        table: table_factor(&j.relation),
        on: join_on(&j.join_operator).map(|e| expr(&e, raw)),
      });
    }
  }
  SelectStmt {
    projections,
    from,
    joins,
    where_clause: s.selection.as_ref().map(|e| expr(e, raw)),
    cte_names: Vec::new(),
  }
}

fn projection(p: &sp::SelectItem, raw: &str) -> Projection {
  match p {
    sp::SelectItem::Wildcard(_) => Projection::Star,
    sp::SelectItem::QualifiedWildcard(name, _) => Projection::QualifiedStar(name.to_string()),
    sp::SelectItem::UnnamedExpr(e) => Projection::Expr { expr: expr(e, raw), alias: None },
    sp::SelectItem::ExprWithAlias { expr: e, alias } => {
      Projection::Expr { expr: expr(e, raw), alias: Some(alias.value.clone()) }
    },
  }
}

fn join_kind(op: &sp::JoinOperator) -> JoinKind {
  match op {
    sp::JoinOperator::Inner(_) => JoinKind::Inner,
    sp::JoinOperator::LeftOuter(_) => JoinKind::Left,
    sp::JoinOperator::RightOuter(_) => JoinKind::Right,
    sp::JoinOperator::FullOuter(_) => JoinKind::Full,
    sp::JoinOperator::CrossJoin => JoinKind::Cross,
    _ => JoinKind::Inner,
  }
}

fn join_on(op: &sp::JoinOperator) -> Option<sp::Expr> {
  match op {
    sp::JoinOperator::Inner(c)
    | sp::JoinOperator::LeftOuter(c)
    | sp::JoinOperator::RightOuter(c)
    | sp::JoinOperator::FullOuter(c) => match c {
      sp::JoinConstraint::On(e) => Some(e.clone()),
      _ => None,
    },
    _ => None,
  }
}

fn table_factor(tf: &sp::TableFactor) -> TableRef {
  match tf {
    sp::TableFactor::Table { name, alias, .. } => {
      let (schema, table_name) = split_object_name(name);
      TableRef {
        schema,
        name: table_name,
        alias: alias.as_ref().map(|a| a.name.value.clone()),
        range: TextRange::default(),
      }
    },
    // `SELECT * FROM generate_series(1, 10) AS number` and friends.
    // The function call IS the table source; we use its return alias
    // (e.g. `number`) as the binding name so the resolver puts it in
    // scope. Sentinel schema `<func>` marks this binding as synthetic
    // so sql001 / sql002 don't try to look it up in the live catalog.
    sp::TableFactor::Function { alias, .. } => {
      let alias_str = alias.as_ref().map(|a| a.name.value.clone());
      TableRef {
        schema: Some("<func>".into()),
        name: alias_str.clone().unwrap_or_default(),
        alias: alias_str,
        range: TextRange::default(),
      }
    },
    sp::TableFactor::TableFunction { alias, .. } | sp::TableFactor::UNNEST { alias, .. } => {
      let alias_str = alias.as_ref().map(|a| a.name.value.clone());
      TableRef {
        schema: Some("<func>".into()),
        name: alias_str.clone().unwrap_or_default(),
        alias: alias_str,
        range: TextRange::default(),
      }
    },
    sp::TableFactor::Derived { alias, .. } => {
      let alias_str = alias.as_ref().map(|a| a.name.value.clone());
      TableRef {
        schema: Some("<subq>".into()),
        name: alias_str.clone().unwrap_or_default(),
        alias: alias_str,
        range: TextRange::default(),
      }
    },
    _ => TableRef::default(),
  }
}

fn object_name(name: &sp::ObjectName) -> TableRef {
  let (schema, table_name) = split_object_name(name);
  TableRef { schema, name: table_name, alias: None, range: TextRange::default() }
}

fn split_object_name(name: &sp::ObjectName) -> (Option<String>, String) {
  let parts: Vec<&str> = name.0.iter().map(|i| i.value.as_str()).collect();
  if parts.len() == 2 { (Some(parts[0].to_string()), parts[1].to_string()) } else { (None, parts.join(".")) }
}

fn column_def(c: &sp::ColumnDef) -> ColumnDef {
  let not_null = c
    .options
    .iter()
    .any(|o| matches!(o.option, sp::ColumnOption::NotNull | sp::ColumnOption::Unique { is_primary: true, .. }));
  let default = c.options.iter().find_map(|o| match &o.option {
    sp::ColumnOption::Default(e) => Some(e.to_string()),
    _ => None,
  });
  ColumnDef {
    name: c.name.value.clone(),
    type_name: c.data_type.to_string(),
    nullable: !not_null,
    default,
    range: TextRange::default(),
  }
}

fn expr(e: &sp::Expr, _raw: &str) -> Expr {
  match e {
    sp::Expr::Identifier(id) => Expr::Column { qualifier: None, name: id.value.clone(), range: TextRange::default() },
    sp::Expr::CompoundIdentifier(parts) => {
      let qual = parts.get(parts.len().saturating_sub(2)).map(|i| i.value.clone());
      let name = parts.last().map(|i| i.value.clone()).unwrap_or_default();
      Expr::Column { qualifier: qual, name, range: TextRange::default() }
    },
    other => Expr::Other(other.to_string()),
  }
}
