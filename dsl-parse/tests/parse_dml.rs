use dsl_parse::{Dialect, StatementKind, parse};

#[test]
fn parses_insert() {
  let p = parse("INSERT INTO users (name, email) VALUES ('alice', 'a@x');", Dialect::Postgres);
  let StatementKind::Insert(i) = &p.statements[0].kind else { panic!() };
  assert_eq!(i.table.name, "users");
  assert_eq!(i.columns, vec!["name", "email"]);
}

#[test]
fn parses_update_with_where() {
  let p = parse("UPDATE users SET active = false WHERE id = 1;", Dialect::Postgres);
  let StatementKind::Update(u) = &p.statements[0].kind else { panic!() };
  assert_eq!(u.table.name, "users");
  assert_eq!(u.assignments.len(), 1);
  assert!(u.where_clause.is_some());
}

#[test]
fn parses_delete_with_where() {
  let p = parse("DELETE FROM sessions WHERE expires_at < now();", Dialect::Postgres);
  let StatementKind::Delete(d) = &p.statements[0].kind else { panic!() };
  assert_eq!(d.table.name, "sessions");
  assert!(d.where_clause.is_some());
}
