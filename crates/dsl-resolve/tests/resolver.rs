use dsl_parse::{parse, Dialect};
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
    let p = parse(
        "SELECT * FROM users u JOIN orders o ON o.user_id = u.id",
        Dialect::Postgres,
    );
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
