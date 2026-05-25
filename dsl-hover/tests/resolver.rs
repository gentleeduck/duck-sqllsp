use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Schema, Table, TableKind};
use dsl_hover::resolver::resolve;

fn cat() -> Catalog {
  let users = Table {
    schema: "public".into(),
    name: "users".into(),
    kind: TableKind::Table,
    columns: vec![
      Column { name: "id".into(), data_type: "uuid".into(), nullable: false, default: None, comment: None, generated: None },
      Column { name: "email".into(), data_type: "text".into(), nullable: false, default: None, comment: None, generated: None },
    ],
    constraints: vec![],
    indexes: vec![],
    triggers: vec![],
    policies: vec![],
    comment: None,
  };
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema { name: "public".into(), tables: vec![users] }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

#[test]
fn resolves_plain_table() {
  let md = resolve("users", &cat()).expect("table found");
  assert!(md.contains("Table"));
  assert!(md.contains("`public.users`"));
  assert!(md.contains("id"));
}

#[test]
fn resolves_schema_dot_table() {
  let md = resolve("public.users", &cat()).expect("qualified table");
  assert!(md.contains("public.users"));
}

#[test]
fn resolves_table_dot_column() {
  let md = resolve("users.email", &cat()).expect("table column");
  assert!(md.contains("Column"));
  assert!(md.contains("email"));
}

#[test]
fn resolves_plain_column() {
  let md = resolve("email", &cat()).expect("column");
  assert!(md.contains("Column"));
}

#[test]
fn resolves_keyword() {
  let md = resolve("SELECT", &Catalog::default()).expect("keyword");
  assert!(md.contains("Retrieve"));
}

#[test]
fn resolves_function() {
  let md = resolve("count", &Catalog::default()).expect("function");
  assert!(md.contains("count(* | expr)"));
}

#[test]
fn resolves_type() {
  let md = resolve("UUID", &Catalog::default()).expect("type");
  assert!(md.contains("gen_random_uuid"));
}

#[test]
fn returns_none_for_unknown_token() {
  assert!(resolve("frobnicate_xyz", &Catalog::default()).is_none());
}
