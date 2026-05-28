use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve;

#[test]
fn binds_alias_in_from() {
  let p = parse("SELECT u.id FROM users u", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some());
  assert_eq!(scopes[0].get("u").unwrap().table.name, "users");
}

#[test]
fn binds_unaliased_by_name() {
  let p = parse("SELECT * FROM users", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("users").is_some());
}

#[test]
fn binds_aliased_table_by_both_alias_and_name() {
  let p = parse("SELECT users.id FROM users u WHERE u.id = 1", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some());
  assert!(scopes[0].get("users").is_some());
}

#[test]
fn binds_join_table() {
  let p = parse("SELECT * FROM users u JOIN orders o ON o.user_id = u.id", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("o").is_some());
  assert_eq!(scopes[0].get("o").unwrap().table.name, "orders");
  assert_eq!(scopes[0].len(), 4);
}

#[test]
fn update_delete_insert_bind_target() {
  let p = parse("UPDATE users SET active = false WHERE id = 1;", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("users").is_some());

  let p = parse("DELETE FROM users WHERE id = 1;", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("users").is_some());

  let p = parse("INSERT INTO users (name) VALUES ('a');", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("users").is_some());
}

#[test]
fn cte_columns_lookup_returns_some_empty_for_declared_cte() {
  let p = parse("WITH t AS (SELECT id, email FROM users) SELECT * FROM t;", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  let cols = scopes[0].cte_columns_of("t");
  assert!(cols.is_some(), "declared CTE should be present");
  assert!(cols.unwrap().is_empty(), "columns not yet populated -- expect Some(empty)");
}

#[test]
fn cte_columns_lookup_returns_none_for_unknown_cte() {
  let p = parse("SELECT * FROM users;", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].cte_columns_of("nope").is_none());
}

#[test]
fn resolve_with_source_extracts_cte_projection_columns() {
  use dsl_resolve::resolve_with_source;
  let src = "WITH t AS (SELECT id, email FROM users) SELECT * FROM t;";
  let p = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&p.statements, src);
  let cols = scopes[0].cte_columns_of("t").expect("t declared");
  assert!(cols.contains(&"id".to_string()), "got {cols:?}");
  assert!(cols.contains(&"email".to_string()), "got {cols:?}");
}

#[test]
fn resolve_with_source_uses_explicit_column_list_when_present() {
  use dsl_resolve::resolve_with_source;
  let src = "WITH t(a, b) AS (SELECT id, email FROM users) SELECT * FROM t;";
  let p = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&p.statements, src);
  let cols = scopes[0].cte_columns_of("t").expect("t declared");
  assert_eq!(cols, &vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn resolve_with_source_handles_alias_with_as() {
  use dsl_resolve::resolve_with_source;
  let src = "WITH t AS (SELECT count(*) AS total FROM users) SELECT * FROM t;";
  let p = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&p.statements, src);
  let cols = scopes[0].cte_columns_of("t").expect("t declared");
  assert!(cols.contains(&"total".to_string()), "got {cols:?}");
}

#[test]
fn resolve_with_source_handles_subquery_in_projection() {
  use dsl_resolve::resolve_with_source;
  let src = "WITH t AS (SELECT (SELECT MAX(x) FROM other) AS m, id FROM users) SELECT * FROM t;";
  let p = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&p.statements, src);
  let cols = scopes[0].cte_columns_of("t").expect("t declared");
  assert!(cols.contains(&"m".to_string()), "subquery alias 'm' missing, got {cols:?}");
  assert!(cols.contains(&"id".to_string()), "trailing 'id' missing, got {cols:?}");
}

#[test]
fn resolve_with_source_handles_window_function_with_partition_by() {
  use dsl_resolve::resolve_with_source;
  let src = "WITH t AS (SELECT id, COUNT(*) OVER (PARTITION BY user_id) AS cnt FROM events) SELECT * FROM t;";
  let p = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&p.statements, src);
  let cols = scopes[0].cte_columns_of("t").expect("t declared");
  assert!(cols.contains(&"id".to_string()), "got {cols:?}");
  assert!(cols.contains(&"cnt".to_string()), "window alias 'cnt' missing, got {cols:?}");
}

#[test]
fn resolve_with_source_handles_string_literal_containing_from() {
  use dsl_resolve::resolve_with_source;
  let src = "WITH t AS (SELECT 'hello FROM world' AS greeting, id FROM users) SELECT * FROM t;";
  let p = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&p.statements, src);
  let cols = scopes[0].cte_columns_of("t").expect("t declared");
  assert!(cols.contains(&"greeting".to_string()), "got {cols:?}");
  assert!(cols.contains(&"id".to_string()), "trailing 'id' missing, got {cols:?}");
}

#[test]
fn resolve_with_source_handles_quoted_identifier_alias() {
  use dsl_resolve::resolve_with_source;
  let src = "WITH t AS (SELECT id AS \"User Id\", email FROM users) SELECT * FROM t;";
  let p = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&p.statements, src);
  let cols = scopes[0].cte_columns_of("t").expect("t declared");
  assert!(cols.iter().any(|c| c == "User Id"), "quoted alias missing, got {cols:?}");
  assert!(cols.contains(&"email".to_string()), "got {cols:?}");
}
