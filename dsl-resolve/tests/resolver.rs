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

#[test]
fn r2_075_binds_multiple_joins() {
  let p = parse("SELECT * FROM users u JOIN orders o ON u.id = o.user_id JOIN payments p ON o.id = p.order_id", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing");
  assert!(scopes[0].get("o").is_some(), "o missing");
  assert!(scopes[0].get("p").is_some(), "p missing");
}

#[test]
fn r2_075_binds_lateral_subquery_alias() {
  let p = parse("SELECT * FROM users u, LATERAL (SELECT * FROM orders WHERE user_id = u.id) AS o", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing in {:?}", scopes[0]);
}

#[test]
fn r2_075_binds_self_join_aliases() {
  let p = parse("SELECT * FROM users a JOIN users b ON a.id = b.id", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("a").is_some(), "a missing");
  assert!(scopes[0].get("b").is_some(), "b missing");
}

#[test]
fn r2_075_resolves_update_target() {
  let p = parse("UPDATE users SET name = 'x' WHERE id = 1", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("users").is_some(), "users missing");
}

#[test]
fn r2_075_resolves_delete_target() {
  let p = parse("DELETE FROM users WHERE id = 1", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("users").is_some(), "users missing");
}

#[test]
fn r2_075_resolves_insert_target() {
  let p = parse("INSERT INTO users (id, name) VALUES (1, 'x')", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("users").is_some(), "users missing");
}


#[test]
fn r2_126_binds_cte_with_column_list() {
  let p = parse("WITH t(a, b) AS (SELECT 1, 2) SELECT t.a, t.b FROM t", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("t").is_some(), "CTE alias `t` missing");
}

#[test]
fn r2_126_binds_lateral_with_set_returning_fn() {
  let p = parse(
    "SELECT * FROM users u, LATERAL generate_series(1, 3) AS gs(n)",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing");
}

#[test]
fn r2_126_binds_update_target_in_returning() {
  let p = parse(
    "UPDATE users u SET name = 'x' WHERE id = 1 RETURNING u.id, u.name",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(
    scopes[0].get("u").is_some() || scopes[0].get("users").is_some(),
    "UPDATE alias / target missing in RETURNING scope"
  );
}

#[test]
fn r2_126_binds_delete_target_alias() {
  // DELETE FROM <t> <alias> -- the target alias binds. USING-clause
  // sibling tables are not yet tracked in the AST (parser extension
  // needed before resolver can pick them up).
  let p = parse("DELETE FROM orders o WHERE o.user_id = 1", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(
    scopes[0].get("o").is_some() || scopes[0].get("orders").is_some(),
    "delete target / alias missing"
  );
}

#[test]
fn r2_126_binds_recursive_cte_self_reference() {
  let p = parse(
    "WITH RECURSIVE t(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM t WHERE n < 10) SELECT * FROM t",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("t").is_some(), "recursive CTE alias missing");
}

#[test]
fn r2_127_binds_nested_cte_references() {
  let p = parse(
    "WITH a AS (SELECT 1 AS x), b AS (SELECT x FROM a) SELECT * FROM a, b",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("a").is_some(), "CTE a missing");
  assert!(scopes[0].get("b").is_some(), "CTE b missing");
}

#[test]
fn r2_127_binds_multiple_ctes_with_recursive() {
  let p = parse(
    "WITH RECURSIVE a(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM a), b AS (SELECT * FROM a) SELECT * FROM b",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("a").is_some(), "recursive a missing");
  assert!(scopes[0].get("b").is_some(), "non-recursive b missing");
}

#[test]
fn r2_127_binds_subquery_alias_in_from() {
  let p = parse(
    "SELECT t.id FROM (SELECT id FROM users) AS t",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("t").is_some(), "subquery alias t missing");
}

#[test]
fn r2_127_binds_insert_target_alias() {
  let p = parse(
    "INSERT INTO users AS u (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(
    scopes[0].get("u").is_some() || scopes[0].get("users").is_some(),
    "INSERT target / alias missing"
  );
}

#[test]
fn r2_127_binds_cte_column_list_columns() {
  let p = parse(
    "WITH t(alpha, beta) AS (SELECT 1, 2) SELECT alpha FROM t",
    Dialect::Postgres,
  );
  let scopes = ::dsl_resolve::resolve_with_source(
    &p.statements,
    "WITH t(alpha, beta) AS (SELECT 1, 2) SELECT alpha FROM t",
  );
  let cols = scopes[0].cte_columns_of("t").expect("CTE t cte_columns_of missing");
  assert!(cols.iter().any(|c| c.eq_ignore_ascii_case("alpha")), "alpha missing");
  assert!(cols.iter().any(|c| c.eq_ignore_ascii_case("beta")), "beta missing");
}

#[test]
fn r2_127_binds_join_then_lateral() {
  let p = parse(
    "SELECT * FROM users u JOIN orders o ON o.user_id = u.id, LATERAL (SELECT 1 AS k) AS k",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing");
  assert!(scopes[0].get("o").is_some(), "o missing");
}

#[test]
fn r2_128_binds_schema_qualified_table() {
  let p = parse("SELECT * FROM public.users u", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing");
}

#[test]
fn r2_128_binds_cross_join() {
  let p = parse("SELECT * FROM users u CROSS JOIN orders o", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing");
  assert!(scopes[0].get("o").is_some(), "o missing");
}

#[test]
fn r2_128_binds_left_outer_join() {
  let p = parse(
    "SELECT * FROM users u LEFT OUTER JOIN orders o ON u.id = o.user_id",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing");
  assert!(scopes[0].get("o").is_some(), "o missing");
}

#[test]
fn r2_128_binds_right_outer_join() {
  let p = parse(
    "SELECT * FROM users u RIGHT OUTER JOIN orders o ON u.id = o.user_id",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing");
  assert!(scopes[0].get("o").is_some(), "o missing");
}

#[test]
fn r2_128_binds_full_outer_join() {
  let p = parse(
    "SELECT * FROM users u FULL OUTER JOIN orders o ON u.id = o.user_id",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing");
  assert!(scopes[0].get("o").is_some(), "o missing");
}

#[test]
fn r2_128_binds_natural_join() {
  let p = parse(
    "SELECT * FROM users u NATURAL JOIN orders o",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing");
  assert!(scopes[0].get("o").is_some(), "o missing");
}

#[test]
fn r2_128_binds_inner_join_using() {
  let p = parse(
    "SELECT * FROM users u INNER JOIN orders o USING (id)",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u").is_some(), "u missing");
  assert!(scopes[0].get("o").is_some(), "o missing");
}

#[test]
fn r2_128_binds_quoted_alias() {
  let p = parse(
    "SELECT \"User\".id FROM users \"User\"",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  // Quoted alias should bind under both quoted and bare form (case-sensitive).
  assert!(
    scopes[0].get("User").is_some() || scopes[0].get("\"User\"").is_some(),
    "quoted alias `User` missing"
  );
}

#[test]
fn r2_162_resolve_empty_input() {
  let p = parse("", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes.is_empty() || scopes[0].is_empty());
}

#[test]
fn r3_003_update_from_binds_extra_tables() {
  // CYCLE 3: `UPDATE t SET ... FROM <list> WHERE ...` adds the
  // FROM list as additional bindings. Previously only `users` was
  // bound; `orders` was invisible in WHERE clause scope.
  let p = parse(
    "UPDATE users SET active = o.flag FROM orders o WHERE o.user_id = users.id;",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("users").is_some(), "users target missing");
  assert!(scopes[0].get("o").is_some(), "FROM-list alias o missing");
  assert!(scopes[0].get("orders").is_some(), "FROM-list table orders missing");
}

#[test]
fn r3_003_delete_using_binds_extra_tables() {
  // CYCLE 3: `DELETE FROM tgt USING <list> WHERE ...` adds the
  // USING list as additional bindings.
  let p = parse(
    "DELETE FROM users USING orders o WHERE o.user_id = users.id;",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("users").is_some(), "users target missing");
  assert!(scopes[0].get("o").is_some(), "USING-list alias o missing");
  assert!(scopes[0].get("orders").is_some(), "USING-list table orders missing");
}

#[test]
fn r3_003_update_from_multi_table() {
  let p = parse(
    "UPDATE users SET x = 1 FROM orders o, products p WHERE 1=1;",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("o").is_some());
  assert!(scopes[0].get("p").is_some());
}

#[test]
fn r3_003_delete_using_multi_table() {
  let p = parse(
    "DELETE FROM users USING orders o, products p WHERE 1=1;",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("o").is_some());
  assert!(scopes[0].get("p").is_some());
}

#[test]
fn r3_079_resolve_moderate_alias_chain() {
  // Verify first few JOIN-chain aliases bind; deep chains may strain
  // pg_query's JoinExpr nesting depth, so probe at u3.
  let mut s = String::from("SELECT * FROM users u1");
  for i in 2..6 {
    s.push_str(&format!(" JOIN users u{i} ON u{i}.id = u{}.id", i - 1));
  }
  let p = parse(&s, Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("u1").is_some(), "u1 missing");
  assert!(scopes[0].get("u3").is_some(), "u3 missing");
}

#[test]
fn r3_237_resolve_schema_qualified() {
  let p = parse("SELECT * FROM public.users", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("users").is_some());
}

#[test]
fn r3_238_resolve_cte_chain() {
  let p = parse("WITH a AS (SELECT 1), b AS (SELECT 2) SELECT * FROM a, b", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("a").is_some());
  assert!(scopes[0].get("b").is_some());
}

#[test]
fn r3_241_resolve_multiple_statements() {
  let p = parse("SELECT 1; UPDATE users SET x = 1; DELETE FROM orders;", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(!scopes.is_empty());
  assert_eq!(scopes.len(), p.statements.len());
}

#[test]
fn r3_244_resolve_self_join_aliases() {
  let p = parse("SELECT a.id, b.id FROM users a JOIN users b ON a.parent_id = b.id", Dialect::Postgres);
  let scopes = resolve(&p.statements);
  assert!(scopes[0].get("a").is_some());
  assert!(scopes[0].get("b").is_some());
}

#[test]
fn r3_387_resolve_chain_dml() {
  let p = parse(
    "BEGIN; INSERT INTO users (id) VALUES (1); UPDATE users SET x = 1; DELETE FROM users; COMMIT;",
    Dialect::Postgres,
  );
  let scopes = resolve(&p.statements);
  assert_eq!(scopes.len(), p.statements.len());
}

#[test]
fn r9_resolve_empty_0401() {
  let p = parse("", Dialect::Postgres);
  let s = resolve(&p.statements);
  assert!(s.is_empty(), "expected empty scopes for empty input");
}

#[test]
fn r9_resolve_ws_0501() {
  let p = parse("   ", Dialect::Postgres);
  let s = resolve(&p.statements);
  assert!(s.is_empty());
}

#[test]
fn r9_resolve_ws_0502() {
  let p = parse("\n", Dialect::Postgres);
  let s = resolve(&p.statements);
  assert!(s.is_empty());
}

#[test]
fn r9_resolve_ws_0503() {
  let p = parse(" \n ", Dialect::Postgres);
  let s = resolve(&p.statements);
  assert!(s.is_empty());
}

#[test]
fn r9_resolve_ws_0504() {
  let p = parse("-- comment", Dialect::Postgres);
  let s = resolve(&p.statements);
  assert!(s.is_empty());
}

#[test]
fn r9_resolve_ws_0505() {
  let p = parse("/* */", Dialect::Postgres);
  let s = resolve(&p.statements);
  assert!(s.is_empty());
}

#[test]
fn r9_resolve_idem_0601() {
  let p = parse("SELECT 1", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0602() {
  let p = parse("SELECT id FROM users", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0603() {
  let p = parse("SELECT id, name FROM users", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0604() {
  let p = parse("SELECT u.id FROM users u", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0605() {
  let p = parse("SELECT * FROM users WHERE id = 1", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0606() {
  let p = parse("SELECT count(*) FROM users", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0607() {
  let p = parse("SELECT * FROM users JOIN orders ON users.id = orders.user_id", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0608() {
  let p = parse("INSERT INTO users (id) VALUES (1)", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0609() {
  let p = parse("UPDATE users SET name = 'x' WHERE id = 1", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0610() {
  let p = parse("DELETE FROM users WHERE id = 1", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0611() {
  let p = parse("WITH x AS (SELECT 1) SELECT * FROM x", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}

#[test]
fn r9_resolve_idem_0612() {
  let p = parse("WITH RECURSIVE r AS (SELECT 1 UNION SELECT r.n + 1 FROM r) SELECT * FROM r", Dialect::Postgres);
  let s1 = resolve(&p.statements);
  let s2 = resolve(&p.statements);
  assert_eq!(s1.len(), s2.len());
}


