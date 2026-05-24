use dsl_parse::{Dialect, StatementKind, parse};

#[test]
fn create_table_collects_columns() {
  let sql = "CREATE TABLE IF NOT EXISTS users (id UUID PRIMARY KEY DEFAULT gen_random_uuid(), email TEXT NOT NULL);";
  let p = parse(sql, Dialect::Postgres);
  assert!(p.errors.is_empty(), "errors: {:?}", p.errors);
  let StatementKind::CreateTable(c) = &p.statements[0].kind else {
    panic!("expected CreateTable, got {:?}", p.statements[0].kind);
  };
  assert!(c.if_not_exists);
  assert_eq!(c.table.name, "users");
  assert_eq!(c.columns.len(), 2);
  assert_eq!(c.columns[0].name, "id");
  assert!(!c.columns[0].nullable, "PK is implicitly NOT NULL");
  assert!(!c.columns[1].nullable, "explicit NOT NULL");
}

#[test]
fn drop_table_collects_targets() {
  let p = parse("DROP TABLE IF EXISTS users, sessions;", Dialect::Postgres);
  let StatementKind::DropTable(d) = &p.statements[0].kind else { panic!() };
  assert!(d.if_exists);
  assert_eq!(d.tables.len(), 2);
}

#[test]
fn alter_table_keeps_target() {
  let p = parse("ALTER TABLE users ADD COLUMN role TEXT;", Dialect::Postgres);
  let StatementKind::AlterTable(a) = &p.statements[0].kind else { panic!() };
  assert_eq!(a.table.name, "users");
}
