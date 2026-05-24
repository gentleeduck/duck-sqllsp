//! Resolver invariants -- hold for every SELECT we can parse.
//!
//! Property 1: every alias key in the scope resolves to the same
//!             binding as its underlying table name key (when the table
//!             is also bound).
//! Property 2: every binding's table.name is non-empty.
//! Property 3: scope is never empty when at least one FROM/JOIN table
//!             is present in the parse.
//! Property 4: aliased binding's `alias` matches the scope key when
//!             accessed by alias.

use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve;

const FIXTURES: &[&str] = &[
  "SELECT 1 FROM users",
  "SELECT 1 FROM users u",
  "SELECT 1 FROM users AS u",
  "SELECT 1 FROM users u, orders o",
  "SELECT 1 FROM users u JOIN orders o ON u.id = o.user_id",
  "SELECT 1 FROM users u LEFT JOIN orders o ON u.id = o.user_id",
  "SELECT 1 FROM s.t",
  "SELECT 1 FROM s.t AS x",
  "WITH cte AS (SELECT 1) SELECT * FROM cte",
  "UPDATE users SET name = 'x' WHERE id = 1",
  "DELETE FROM users WHERE id = 1",
  "INSERT INTO users (id, email) VALUES (1, 'a@b.com')",
];

#[test]
fn property_all_bindings_have_nonempty_table_name() {
  for src in FIXTURES {
    let p = parse(src, Dialect::Postgres);
    for scope in resolve(&p.statements) {
      for b in scope.tables() {
        assert!(!b.table.name.is_empty(), "binding with empty table.name in `{src}`");
      }
    }
  }
}

#[test]
fn property_alias_lookup_returns_same_table_as_name_lookup() {
  for src in FIXTURES {
    let p = parse(src, Dialect::Postgres);
    for scope in resolve(&p.statements) {
      let names: Vec<String> = scope.bindings.keys().cloned().collect();
      for n in &names {
        let Some(b) = scope.get(n) else { continue };
        // Look up by the binding's own table name; should be a
        // valid binding too.
        let tname = &b.table.name;
        if let Some(b2) = scope.get(tname) {
          assert_eq!(b.table.name, b2.table.name, "alias `{n}` and name `{tname}` disagree in `{src}`");
        }
      }
    }
  }
}

#[test]
fn property_select_from_table_yields_nonempty_scope() {
  for src in FIXTURES.iter().filter(|s| s.starts_with("SELECT") || s.starts_with("WITH")) {
    let p = parse(src, Dialect::Postgres);
    let scopes = resolve(&p.statements);
    for scope in &scopes {
      assert!(!scope.is_empty(), "scope empty for `{src}` (parsed={:?})", scopes.len());
    }
  }
}

#[test]
fn property_alias_key_matches_binding_alias() {
  let p = parse("SELECT * FROM users u JOIN orders o ON true", Dialect::Postgres);
  let s = &resolve(&p.statements)[0];
  let u = s.get("u").expect("u bound");
  assert_eq!(u.alias, "u");
  let o = s.get("o").expect("o bound");
  assert_eq!(o.alias, "o");
}

#[test]
fn property_cte_name_is_synthetic_binding() {
  let p = parse("WITH cte AS (SELECT 1) SELECT * FROM cte", Dialect::Postgres);
  let s = &resolve(&p.statements)[0];
  assert!(s.get("cte").is_some(), "CTE not bound");
}
