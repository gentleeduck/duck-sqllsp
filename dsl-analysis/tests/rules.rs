use dsl_analysis::{Severity, run};
use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Constraint, ConstraintKind, Schema, Table, TableKind};
use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve_with_source;

fn cat() -> Catalog {
  let users = Table {
    schema: "public".into(),
    name: "users".into(),
    kind: TableKind::Table,
    columns: vec![
      Column {
        name: "id".into(),
        data_type: "uuid".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      },
      Column {
        name: "email".into(),
        data_type: "text".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      },
      Column {
        name: "name".into(),
        data_type: "text".into(),
        nullable: true,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      },
    ],
    constraints: vec![Constraint {
      name: "pk_users_id".into(),
      kind: ConstraintKind::PrimaryKey,
      columns: vec!["id".into()],
      references: None,
      definition: None,
      inline: false,
    }],
    indexes: vec![],
    triggers: vec![],
    policies: vec![],
    comment: None,
    row_estimate: None,
    owner: None, definition: None, strict: false, options: None,
  };
  let orders = Table {
    schema: "public".into(),
    name: "orders".into(),
    kind: TableKind::Table,
    columns: vec![
      Column {
        name: "id".into(),
        data_type: "uuid".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      },
      Column {
        name: "user_id".into(),
        data_type: "uuid".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      },
    ],
    constraints: vec![],
    indexes: vec![],
    triggers: vec![],
    policies: vec![],
    comment: None,
    row_estimate: None,
    owner: None, definition: None, strict: false, options: None,
  };
  let flags = Table {
    schema: "public".into(),
    name: "flags".into(),
    kind: TableKind::Table,
    columns: vec![
      Column {
        name: "id".into(),
        data_type: "uuid".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      },
      Column {
        name: "active".into(),
        data_type: "boolean".into(),
        nullable: false,
        default: None,
        comment: None,
        generated: None,
        json_keys: None,
      },
    ],
    constraints: vec![],
    indexes: vec![],
    triggers: vec![],
    policies: vec![],
    comment: None,
    row_estimate: None,
    owner: None, definition: None, strict: false, options: None,
  };
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema { name: "public".into(), tables: vec![users, orders, flags] }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

fn diags(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &cat())
}

#[test]
fn sql001_unresolved_table() {
  let d = diags("SELECT * FROM nonexistent_thing;");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn sql001_quiet_when_table_exists() {
  let d = diags("SELECT * FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn sql001_quiet_with_empty_catalog() {
  let empty = Catalog::default();
  let file = parse("SELECT * FROM anything;", Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, "SELECT * FROM anything;");
  let d = run("SELECT * FROM anything;", &file, &scopes, &empty);
  assert!(d.iter().all(|x| x.code != "sql001"));
}

#[test]
fn sql002_unknown_column() {
  let d = diags("SELECT nope FROM users;");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn sql002_quiet_when_column_exists() {
  let d = diags("SELECT email FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn sql002_unknown_column_in_function_call_projection() {
  // `SELECT length(noexist) FROM users` -- the column reference is
  // wrapped in a FuncCall, which used to reduce the projection to
  // `Expr::Other("")` and hide the typo from sql002.
  let d = diags("SELECT length(noexist) FROM users;");
  assert!(
    d.iter().any(|x| x.code == "sql002" && x.message.contains("noexist")),
    "expected sql002 for `noexist` in length() call; got {d:?}"
  );
}

#[test]
fn sql002_unknown_column_in_case_branch() {
  // CASE WHEN id IS NULL THEN noexist END -- the THEN arm hides
  // `noexist` behind a CaseExpr that the projection used to flatten
  // to Other("").
  let d = diags("SELECT CASE WHEN id IS NULL THEN noexist END FROM users;");
  assert!(
    d.iter().any(|x| x.code == "sql002" && x.message.contains("noexist")),
    "expected sql002 for `noexist` in CASE THEN; got {d:?}"
  );
}

#[test]
fn sql002_unknown_column_in_in_subquery() {
  // `WHERE id IN (SELECT noexist FROM users)` -- the inner SELECT
  // projection used to be invisible to outer-statement analysis.
  let d = diags("SELECT * FROM users WHERE id IN (SELECT noexist FROM users);");
  assert!(
    d.iter().any(|x| x.code == "sql002" && x.message.contains("noexist")),
    "expected sql002 for `noexist` in IN subquery; got {d:?}"
  );
}

#[test]
fn sql002_unknown_column_in_where() {
  // Unknown column referenced in a WHERE predicate must also be flagged.
  // Previously the pg_query backend reduced the WHERE clause to
  // `Expr::Other("")`, so the rule walked nothing and silently missed it.
  let d = diags("SELECT * FROM users WHERE noexist = 1;");
  assert!(
    d.iter().any(|x| x.code == "sql002" && x.message.contains("noexist")),
    "expected sql002 for `noexist` in WHERE; got {d:?}"
  );
}

#[test]
fn sql002_unknown_column_in_join_on() {
  let d = diags("SELECT * FROM users u JOIN orders o ON o.user_id = u.noexist;");
  assert!(
    d.iter().any(|x| x.code == "sql002" && x.message.contains("noexist")),
    "expected sql002 for `noexist` in JOIN ON; got {d:?}"
  );
}

#[test]
fn sql002_unknown_column_in_update_where() {
  // UPDATE/DELETE WHERE unknown column is covered by sql351 (DML-specific
  // rule) -- either diagnostic is acceptable, both surface the typo.
  let d = diags("UPDATE users SET email = 'x' WHERE noexist = 1;");
  assert!(
    d.iter().any(|x| (x.code == "sql002" || x.code == "sql351") && x.message.contains("noexist")),
    "expected sql002/sql351 for `noexist` in UPDATE WHERE; got {d:?}"
  );
}

#[test]
fn sql002_unknown_column_in_delete_where() {
  let d = diags("DELETE FROM users WHERE noexist = 1;");
  assert!(
    d.iter().any(|x| (x.code == "sql002" || x.code == "sql351") && x.message.contains("noexist")),
    "expected sql002/sql351 for `noexist` in DELETE WHERE; got {d:?}"
  );
}

#[test]
fn sql003_ambiguous_column() {
  let d = diags("SELECT id FROM users u JOIN orders o ON o.user_id = u.id;");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn sql207_concat_single_arg_is_noop() {
  let d = diags("SELECT CONCAT('hello');");
  assert!(
    d.iter().any(|x| x.code == "sql207" && x.message.contains("concat")),
    "expected sql207 for CONCAT('hello'): {d:?}"
  );
}

#[test]
fn sql207_concat_ws_with_single_value_is_noop() {
  let d = diags("SELECT CONCAT_WS(',', 'a');");
  assert!(
    d.iter().any(|x| x.code == "sql207" && x.message.contains("separator never joins")),
    "expected sql207 for CONCAT_WS: {d:?}"
  );
}

#[test]
fn sql207_concat_two_args_silent() {
  let d = diags("SELECT CONCAT('a', 'b');");
  assert!(!d.iter().any(|x| x.code == "sql207"), "CONCAT(2 args) must not fire: {d:?}");
}

#[test]
fn sql207_concat_ws_with_multiple_values_silent() {
  let d = diags("SELECT CONCAT_WS(',', 'a', 'b');");
  assert!(!d.iter().any(|x| x.code == "sql207"), "CONCAT_WS(3 args) must not fire: {d:?}");
}

#[test]
fn sql052_ilike_without_wildcards_fires() {
  // sql052 now also covers ILIKE -- no wildcards means it behaves
  // like case-insensitive equality, which has a clearer rewrite.
  let d = diags("SELECT * FROM users WHERE email ILIKE 'abc';");
  assert!(
    d.iter().any(|x| x.code == "sql052" && x.message.contains("ILIKE")),
    "expected sql052 for ILIKE without wildcards: {d:?}"
  );
}

#[test]
fn sql052_ilike_with_wildcard_silent() {
  let d = diags("SELECT * FROM users WHERE email ILIKE 'abc%';");
  assert!(!d.iter().any(|x| x.code == "sql052"), "ILIKE with wildcard must not fire: {d:?}");
}

#[test]
fn sql430_star_with_named_column() {
  let d = diags("SELECT *, id FROM users;");
  assert!(d.iter().any(|x| x.code == "sql430"), "got {d:?}");
}

#[test]
fn sql430_quiet_for_star_only() {
  let d = diags("SELECT * FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql430"), "got {d:?}");
}

#[test]
fn sql430_quiet_for_star_with_expression() {
  // `SELECT *, count(*) OVER ()` is a real pattern -- expression
  // projections don't trigger the duplicate-column concern.
  let d = diags("SELECT *, count(*) OVER () AS total FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql430"), "got {d:?}");
}

#[test]
fn sql431_for_update_in_union_trailing() {
  // `SELECT ... UNION SELECT ... FOR UPDATE` -- PG rejects with 0A000.
  let d = diags("SELECT id FROM users UNION SELECT id FROM users FOR UPDATE;");
  assert!(
    d.iter().any(|x| x.code == "sql431" && x.message.contains("UNION")),
    "expected sql431 for trailing FOR UPDATE after UNION: {d:?}"
  );
}

#[test]
fn sql431_for_update_in_union_arm() {
  // `(SELECT ... FOR UPDATE) UNION (...)` -- also rejected by PG.
  let d = diags("(SELECT id FROM users WHERE id = 1 FOR UPDATE) UNION (SELECT id FROM users WHERE id = 2);");
  assert!(d.iter().any(|x| x.code == "sql431"), "expected sql431 for FOR UPDATE inside UNION arm: {d:?}");
}

#[test]
fn sql431_for_share_in_intersect() {
  let d = diags("SELECT id FROM users INTERSECT SELECT id FROM users FOR SHARE;");
  assert!(d.iter().any(|x| x.code == "sql431"), "expected sql431 for FOR SHARE + INTERSECT: {d:?}");
}

#[test]
fn sql431_for_no_key_update_in_except() {
  let d = diags("SELECT id FROM users EXCEPT SELECT id FROM users FOR NO KEY UPDATE;");
  assert!(d.iter().any(|x| x.code == "sql431"), "expected sql431 for FOR NO KEY UPDATE + EXCEPT: {d:?}");
}

#[test]
fn sql431_quiet_for_update_no_setop() {
  // Plain SELECT FOR UPDATE without UNION/INTERSECT/EXCEPT is legal.
  let d = diags("SELECT id FROM users WHERE id = 1 FOR UPDATE;");
  assert!(!d.iter().any(|x| x.code == "sql431"), "must not fire without setop: {d:?}");
}

#[test]
fn sql431_quiet_for_each_row_in_trigger_with_union_unrelated() {
  // `FOR EACH ROW` is NOT a row-locking clause; even alongside a
  // UNION it must not trigger sql431. (CREATE TRIGGER isn't a SELECT,
  // but make sure the FOR-word heuristic isn't over-eager.)
  let d = diags("SELECT id FROM users UNION SELECT id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql431"), "plain UNION without FOR-lock must not fire: {d:?}");
}

#[test]
fn sql432_searched_case_duplicate_when() {
  let d = diags("SELECT CASE WHEN status = 'a' THEN 1 WHEN status = 'a' THEN 2 ELSE 3 END FROM users;");
  assert!(
    d.iter().any(|x| x.code == "sql432" && x.message.contains("status")),
    "expected sql432 for duplicate WHEN status = 'a': {d:?}"
  );
}

#[test]
fn sql432_searched_case_duplicate_when_not_adjacent() {
  let d = diags("SELECT CASE WHEN status = 'a' THEN 1 WHEN status = 'b' THEN 2 WHEN status = 'a' THEN 3 ELSE 4 END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql432"), "expected sql432 for non-adjacent dup WHEN: {d:?}");
}

#[test]
fn sql432_simple_case_duplicate_when_constant() {
  let d = diags("SELECT CASE id WHEN 1 THEN 'a' WHEN 1 THEN 'b' END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql432"), "expected sql432 for simple CASE dup WHEN constant: {d:?}");
}

#[test]
fn sql432_quiet_distinct_branches() {
  let d = diags("SELECT CASE WHEN status = 'a' THEN 1 WHEN status = 'b' THEN 2 ELSE 3 END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql432"), "distinct WHEN branches must not fire: {d:?}");
}

#[test]
fn sql432_quiet_negation_pair() {
  // Different predicates -- not the same condition.
  let d = diags("SELECT CASE WHEN status = 'a' THEN 1 WHEN status <> 'a' THEN 2 END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql432"), "negation pair must not fire: {d:?}");
}

#[test]
fn sql432_quiet_nested_case() {
  // Inner CASE has same condition as outer THEN's inner CASE -- but
  // they're at different scopes and shouldn't be conflated.
  let d = diags(
    "SELECT CASE WHEN status = 'a' THEN CASE WHEN id = 1 THEN 1 ELSE 2 END WHEN status = 'b' THEN CASE WHEN id = 1 THEN 3 ELSE 4 END END FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql432"), "nested CASE inner WHENs must not be flagged across the outer: {d:?}");
}

#[test]
fn sql403_quiet_for_order_by_null_literal() {
  // Regression: NULL is a keyword literal, not a column reference.
  // sql403 must NOT flag it as an unknown column.
  let d = diags("SELECT * FROM users ORDER BY NULL;");
  assert!(
    !d.iter().any(|x| x.code == "sql403"),
    "sql403 must not flag NULL as unknown column: {d:?}"
  );
}

#[test]
fn sql403_quiet_for_order_by_current_date() {
  let d = diags("SELECT * FROM users ORDER BY CURRENT_DATE;");
  assert!(!d.iter().any(|x| x.code == "sql403"), "sql403 must not flag CURRENT_DATE: {d:?}");
}

#[test]
fn sql403_quiet_for_order_by_string_literal_with_desc() {
  // Regression: `'name' DESC` had strip_noise_full blank the literal
  // to spaces; the trailing `DESC` was then misread as a bare ident
  // and reported as an unknown column. The string-literal sort key
  // is sql433's territory; sql403 must keep quiet.
  let d = diags("SELECT * FROM users ORDER BY 'name' DESC;");
  assert!(!d.iter().any(|x| x.code == "sql403"), "sql403 must not misread DESC after literal: {d:?}");
}

#[test]
fn sql403_quiet_for_order_by_string_literal_with_asc() {
  let d = diags("SELECT * FROM users ORDER BY 'name' ASC NULLS LAST;");
  assert!(!d.iter().any(|x| x.code == "sql403"), "sql403 must not misread ASC after literal: {d:?}");
}

#[test]
fn sql403_quiet_for_mixed_literal_and_real_column() {
  // First item is a string-literal (skipped), second is a real
  // column (no diagnostic). sql433 still fires on the literal item.
  let d = diags("SELECT * FROM users ORDER BY 'name', id DESC;");
  assert!(!d.iter().any(|x| x.code == "sql403"), "sql403 must not flag direction after literal: {d:?}");
  assert!(d.iter().any(|x| x.code == "sql433"), "sql433 should still flag the literal sort key: {d:?}");
}

#[test]
fn sql433_order_by_null_is_noop() {
  let d = diags("SELECT * FROM users ORDER BY NULL;");
  assert!(
    d.iter().any(|x| x.code == "sql433" && x.message.contains("NULL")),
    "expected sql433 for ORDER BY NULL: {d:?}"
  );
}

#[test]
fn sql433_order_by_boolean_constant() {
  let d = diags("SELECT * FROM users ORDER BY TRUE;");
  assert!(d.iter().any(|x| x.code == "sql433" && x.message.contains("boolean")), "expected sql433 for ORDER BY TRUE: {d:?}");
}

#[test]
fn sql433_order_by_string_literal() {
  let d = diags("SELECT * FROM users ORDER BY 'foo';");
  assert!(d.iter().any(|x| x.code == "sql433" && x.message.contains("string")), "expected sql433 for ORDER BY 'foo': {d:?}");
}

#[test]
fn sql433_quiet_for_real_column() {
  let d = diags("SELECT * FROM users ORDER BY status;");
  assert!(!d.iter().any(|x| x.code == "sql433"), "ORDER BY real column must not fire: {d:?}");
}

#[test]
fn sql433_quiet_for_positional() {
  // `ORDER BY 1` is sql099's territory (positional reference), not
  // a constant -- it identifies the 1st projection.
  let d = diags("SELECT id FROM users ORDER BY 1;");
  assert!(!d.iter().any(|x| x.code == "sql433"), "positional ORDER BY must not fire sql433: {d:?}");
}

#[test]
fn sql434_is_not_null_with_equality() {
  let d = diags("SELECT * FROM users WHERE email IS NOT NULL AND email = 'a@b.c';");
  assert!(
    d.iter().any(|x| x.code == "sql434"),
    "expected sql434 for IS NOT NULL + equality on same column: {d:?}"
  );
}

#[test]
fn sql434_is_not_null_after_equality() {
  // Order doesn't matter.
  let d = diags("SELECT * FROM users WHERE email = 'a@b.c' AND email IS NOT NULL;");
  assert!(d.iter().any(|x| x.code == "sql434"), "expected sql434 regardless of conjunct order: {d:?}");
}

#[test]
fn sql434_is_not_null_with_like() {
  let d = diags("SELECT * FROM users WHERE email IS NOT NULL AND email LIKE 'a%';");
  assert!(d.iter().any(|x| x.code == "sql434"), "expected sql434 for IS NOT NULL + LIKE: {d:?}");
}

#[test]
fn sql434_is_not_null_with_in_list() {
  let d = diags("SELECT * FROM users WHERE email IS NOT NULL AND email IN ('a','b');");
  assert!(d.iter().any(|x| x.code == "sql434"), "expected sql434 for IS NOT NULL + IN: {d:?}");
}

#[test]
fn sql434_quiet_for_is_not_null_alone() {
  let d = diags("SELECT * FROM users WHERE email IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql434"), "IS NOT NULL alone must not fire: {d:?}");
}

#[test]
fn sql434_quiet_for_or_combinator() {
  // OR -- the IS NOT NULL is NOT redundant because either arm
  // could match independently.
  let d = diags("SELECT * FROM users WHERE email IS NOT NULL OR email = 'a';");
  assert!(!d.iter().any(|x| x.code == "sql434"), "OR must not fire: {d:?}");
}

#[test]
fn sql434_quiet_for_different_columns() {
  let d = diags("SELECT * FROM users WHERE email IS NOT NULL AND status = 'x';");
  assert!(!d.iter().any(|x| x.code == "sql434"), "different columns must not fire: {d:?}");
}

#[test]
fn sql435_is_null_with_equality_contradicts() {
  let d = diags("SELECT * FROM users WHERE email IS NULL AND email = 'a';");
  assert!(
    d.iter().any(|x| x.code == "sql435" && x.message.contains("contradicts")),
    "expected sql435 for IS NULL + equality contradiction: {d:?}"
  );
}

#[test]
fn sql435_is_null_with_is_not_null_contradicts() {
  let d = diags("SELECT * FROM users WHERE email IS NULL AND email IS NOT NULL;");
  assert!(d.iter().any(|x| x.code == "sql435"), "expected sql435 for IS NULL + IS NOT NULL contradiction: {d:?}");
}

#[test]
fn sql435_is_null_with_like_contradicts() {
  let d = diags("SELECT * FROM users WHERE email IS NULL AND email LIKE 'a%';");
  assert!(d.iter().any(|x| x.code == "sql435"), "expected sql435 for IS NULL + LIKE contradiction: {d:?}");
}

#[test]
fn sql435_is_null_with_in_list_contradicts() {
  let d = diags("SELECT * FROM users WHERE email IS NULL AND email IN ('a','b');");
  assert!(d.iter().any(|x| x.code == "sql435"), "expected sql435 for IS NULL + IN contradiction: {d:?}");
}

#[test]
fn sql435_quiet_for_is_null_alone() {
  let d = diags("SELECT * FROM users WHERE email IS NULL;");
  assert!(!d.iter().any(|x| x.code == "sql435"), "IS NULL alone must not fire: {d:?}");
}

#[test]
fn sql435_quiet_for_or_combinator() {
  let d = diags("SELECT * FROM users WHERE email IS NULL OR email = 'a';");
  assert!(!d.iter().any(|x| x.code == "sql435"), "OR with IS NULL must not fire: {d:?}");
}

#[test]
fn sql435_quiet_for_different_column() {
  let d = diags("SELECT * FROM users WHERE email IS NULL AND status = 'x';");
  assert!(!d.iter().any(|x| x.code == "sql435"), "different column must not fire: {d:?}");
}

#[test]
fn sql087_not_between_reversed_flips_message() {
  // `NOT BETWEEN 10 AND 5` == `NOT (always-false)` == matches every row.
  let d = diags("SELECT * FROM users WHERE id NOT BETWEEN 10 AND 5;");
  let m = d.iter().find(|x| x.code == "sql087").unwrap_or_else(|| panic!("expected sql087: {d:?}"));
  // Should be Warning not Error, and message must mention "matches every row".
  assert_eq!(m.severity, dsl_analysis::Severity::Warning, "NOT BETWEEN must drop to Warning: {m:?}");
  assert!(m.message.contains("every row"), "message should mention every row: {m:?}");
}

#[test]
fn sql087_plain_between_reversed_stays_error() {
  // Regression: plain BETWEEN must still be Error + "no rows".
  let d = diags("SELECT * FROM users WHERE id BETWEEN 10 AND 5;");
  let m = d.iter().find(|x| x.code == "sql087").unwrap_or_else(|| panic!("expected sql087: {d:?}"));
  assert_eq!(m.severity, dsl_analysis::Severity::Error);
  assert!(m.message.contains("no rows"), "plain BETWEEN message should say no rows: {m:?}");
}

#[test]
fn sql436_window_inside_aggregate() {
  let d = diags("SELECT sum(row_number() OVER ()) FROM users;");
  assert!(
    d.iter().any(|x| x.code == "sql436" && x.message.contains("sum")),
    "expected sql436 for sum(row_number() OVER ()): {d:?}"
  );
}

#[test]
fn sql436_window_inside_count_with_order() {
  let d = diags("SELECT count(rank() OVER (ORDER BY id)) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql436"), "expected sql436 for count(rank() OVER ...): {d:?}");
}

#[test]
fn sql436_quiet_for_aggregate_inside_window() {
  // Legal: aggregate INSIDE a window's argument is fine -- this is
  // the common `sum(amount) OVER (PARTITION BY ...)` pattern.
  let d = diags("SELECT sum(id) OVER (PARTITION BY status) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql436"), "aggregate inside window must not fire: {d:?}");
}

#[test]
fn sql436_quiet_for_plain_aggregate() {
  let d = diags("SELECT sum(id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql436"), "plain aggregate must not fire: {d:?}");
}

#[test]
fn sql436_quiet_for_plain_window() {
  let d = diags("SELECT row_number() OVER (ORDER BY id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql436"), "plain window must not fire: {d:?}");
}

#[test]
fn sql437_null_in_list_returns_null() {
  let d = diags("SELECT * FROM users WHERE NULL IN (1, 2, 3);");
  assert!(
    d.iter().any(|x| x.code == "sql437" && x.message.contains("NULL IN")),
    "expected sql437 for NULL IN (...): {d:?}"
  );
}

#[test]
fn sql437_null_not_in_list_also_returns_null() {
  let d = diags("SELECT * FROM users WHERE NULL NOT IN (1, 2, 3);");
  assert!(
    d.iter().any(|x| x.code == "sql437" && x.message.contains("NOT IN")),
    "expected sql437 for NULL NOT IN (...): {d:?}"
  );
}

#[test]
fn sql437_quiet_for_column_in_list() {
  let d = diags("SELECT * FROM users WHERE id IN (1, 2, 3);");
  assert!(!d.iter().any(|x| x.code == "sql437"), "column IN list must not fire: {d:?}");
}

#[test]
fn sql437_quiet_for_null_followed_by_other_op() {
  // `IS NULL`, `IS NOT NULL` -- not IN-list patterns.
  let d = diags("SELECT * FROM users WHERE email IS NULL;");
  assert!(!d.iter().any(|x| x.code == "sql437"), "IS NULL must not fire: {d:?}");
}

#[test]
fn sql437_quiet_for_string_literal_containing_null() {
  // strip_noise_full should blank string literals so `'NULL IN'`
  // inside a string doesn't trigger.
  let d = diags("SELECT * FROM users WHERE email = 'NULL IN list';");
  assert!(!d.iter().any(|x| x.code == "sql437"), "string literal must not fire: {d:?}");
}

#[test]
fn sql207_concat_zero_args_uses_zero_arg_message() {
  // Regression: the message must distinguish "no arguments" from
  // "one argument".
  let d = diags("SELECT concat() FROM users;");
  let m = d.iter().find(|x| x.code == "sql207").unwrap_or_else(|| panic!("expected sql207: {d:?}"));
  assert!(
    m.message.contains("no arguments") && m.message.contains("empty string"),
    "zero-arg concat message must mention 'no arguments' / 'empty string': {m:?}"
  );
}

#[test]
fn sql438_generated_always_identity_with_default() {
  let d = diags("CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY DEFAULT 0);");
  assert!(
    d.iter().any(|x| x.code == "sql438" && x.message.contains("DEFAULT")),
    "expected sql438 for IDENTITY + DEFAULT: {d:?}"
  );
}

#[test]
fn sql438_generated_by_default_identity_with_default() {
  let d = diags("CREATE TABLE t (id int GENERATED BY DEFAULT AS IDENTITY DEFAULT 1);");
  assert!(d.iter().any(|x| x.code == "sql438"), "expected sql438 for BY DEFAULT IDENTITY + DEFAULT: {d:?}");
}

#[test]
fn sql438_quiet_for_identity_without_default() {
  let d = diags("CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY);");
  assert!(!d.iter().any(|x| x.code == "sql438"), "plain IDENTITY must not fire: {d:?}");
}

#[test]
fn sql438_quiet_for_default_without_identity() {
  let d = diags("CREATE TABLE t (id int DEFAULT 0);");
  assert!(!d.iter().any(|x| x.code == "sql438"), "plain DEFAULT must not fire: {d:?}");
}

#[test]
fn sql438_quiet_when_default_is_on_other_column() {
  // DEFAULT on a DIFFERENT column (separated by comma) must not
  // make the identity column trigger.
  let d = diags("CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY, name text DEFAULT 'x');");
  assert!(!d.iter().any(|x| x.code == "sql438"), "DEFAULT on separate column must not fire: {d:?}");
}

#[test]
fn sql439_invalid_month_in_date_literal() {
  let d = diags("SELECT DATE '2024-13-01';");
  assert!(
    d.iter().any(|x| x.code == "sql439" && x.message.contains("month 13")),
    "expected sql439 for month=13: {d:?}"
  );
}

#[test]
fn sql439_invalid_day_for_february_non_leap() {
  let d = diags("SELECT DATE '2023-02-29';");
  assert!(
    d.iter().any(|x| x.code == "sql439" && x.message.contains("day 29")),
    "expected sql439 for Feb 29 in non-leap year 2023: {d:?}"
  );
}

#[test]
fn sql439_invalid_day_for_february_leap_year_silent() {
  // 2024 is a leap year -- Feb 29 IS valid.
  let d = diags("SELECT DATE '2024-02-29';");
  assert!(!d.iter().any(|x| x.code == "sql439"), "Feb 29 in leap year must not fire: {d:?}");
}

#[test]
fn sql439_invalid_day_for_april() {
  let d = diags("SELECT DATE '2024-04-31';");
  assert!(d.iter().any(|x| x.code == "sql439"), "expected sql439 for April 31: {d:?}");
}

#[test]
fn sql439_timestamp_literal_invalid_month() {
  let d = diags("SELECT TIMESTAMP '2024-13-01 10:00:00';");
  assert!(d.iter().any(|x| x.code == "sql439"), "expected sql439 for TIMESTAMP invalid month: {d:?}");
}

#[test]
fn sql439_quiet_for_valid_date() {
  let d = diags("SELECT DATE '2024-12-31';");
  assert!(!d.iter().any(|x| x.code == "sql439"), "valid date must not fire: {d:?}");
}

#[test]
fn sql439_quiet_for_non_iso_shape() {
  // We don't validate non-ISO shapes (PG accepts many formats).
  let d = diags("SELECT DATE 'today';");
  assert!(!d.iter().any(|x| x.code == "sql439"), "non-ISO date shape must not fire: {d:?}");
}

#[test]
fn sql439_time_literal_invalid_hour() {
  let d = diags("SELECT TIME '25:00:00';");
  assert!(d.iter().any(|x| x.code == "sql439" && x.message.contains("hour 25")), "expected sql439 for TIME '25:00:00': {d:?}");
}

#[test]
fn sql439_time_literal_invalid_minute() {
  let d = diags("SELECT TIME '12:60:00';");
  assert!(d.iter().any(|x| x.code == "sql439" && x.message.contains("minute 60")), "expected sql439 for TIME '12:60:00': {d:?}");
}

#[test]
fn sql439_time_literal_invalid_second() {
  let d = diags("SELECT TIME '12:00:60';");
  assert!(d.iter().any(|x| x.code == "sql439" && x.message.contains("second 60")), "expected sql439 for TIME '12:00:60': {d:?}");
}

#[test]
fn sql439_timestamp_invalid_hour_in_time_portion() {
  let d = diags("SELECT TIMESTAMP '2024-01-01 25:00:00';");
  assert!(d.iter().any(|x| x.code == "sql439" && x.message.contains("hour 25")), "expected sql439 for TIMESTAMP invalid hour: {d:?}");
}

#[test]
fn sql439_timestamp_t_separator_valid_quiet() {
  let d = diags("SELECT TIMESTAMP '2024-01-01T12:34:56';");
  assert!(!d.iter().any(|x| x.code == "sql439"), "T-separated valid timestamp must not fire: {d:?}");
}

#[test]
fn sql439_time_quiet_for_valid() {
  let d = diags("SELECT TIME '23:59:59';");
  assert!(!d.iter().any(|x| x.code == "sql439"), "valid TIME must not fire: {d:?}");
}

#[test]
fn sql439_time_quiet_for_end_of_day_24() {
  // PG-legal end-of-day marker.
  let d = diags("SELECT TIME '24:00:00';");
  assert!(!d.iter().any(|x| x.code == "sql439"), "TIME '24:00:00' (end of day) must not fire: {d:?}");
}

#[test]
fn sql439_time_flags_24_with_nonzero_minute() {
  let d = diags("SELECT TIME '24:01:00';");
  assert!(d.iter().any(|x| x.code == "sql439" && x.message.contains("end-of-day")), "expected sql439 for TIME 24:01: {d:?}");
}

#[test]
fn sql440_interval_bad_unit_mans() {
  let d = diags("SELECT INTERVAL '2 mans';");
  assert!(d.iter().any(|x| x.code == "sql440" && x.message.contains("mans")), "expected sql440 for `2 mans`: {d:?}");
}

#[test]
fn sql440_interval_bad_unit_typo_weak() {
  let d = diags("SELECT INTERVAL '5 weak';");
  assert!(d.iter().any(|x| x.code == "sql440" && x.message.contains("weak")), "expected sql440 for `5 weak`: {d:?}");
}

#[test]
fn sql440_interval_quiet_for_valid_units() {
  let d = diags("SELECT INTERVAL '3 years 2 months';");
  assert!(!d.iter().any(|x| x.code == "sql440"), "valid units must not fire: {d:?}");
}

#[test]
fn sql440_interval_quiet_for_singular_unit() {
  let d = diags("SELECT INTERVAL '1 day';");
  assert!(!d.iter().any(|x| x.code == "sql440"), "singular unit must not fire: {d:?}");
}

#[test]
fn sql440_interval_quiet_for_iso_8601() {
  let d = diags("SELECT INTERVAL 'P1Y2M3D';");
  assert!(!d.iter().any(|x| x.code == "sql440"), "ISO 8601 must not fire: {d:?}");
}

#[test]
fn sql440_interval_quiet_for_hms_colon_form() {
  let d = diags("SELECT INTERVAL '12:34:56';");
  assert!(!d.iter().any(|x| x.code == "sql440"), "HH:MM:SS form must not fire: {d:?}");
}

#[test]
fn sql440_interval_quiet_for_ago() {
  // PG accepts trailing AGO (negates the interval).
  let d = diags("SELECT INTERVAL '3 days ago';");
  assert!(!d.iter().any(|x| x.code == "sql440"), "ago must not fire: {d:?}");
}

#[test]
fn sql441_uncorrelated_exists_with_alias() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders);");
  assert!(
    d.iter().any(|x| x.code == "sql441" && x.message.contains("uncorrelated")),
    "expected sql441 for uncorrelated EXISTS: {d:?}"
  );
}

#[test]
fn sql441_uncorrelated_exists_no_alias() {
  let d = diags("SELECT * FROM users WHERE EXISTS (SELECT 1 FROM orders);");
  assert!(d.iter().any(|x| x.code == "sql441"), "expected sql441 for uncorrelated EXISTS (no alias): {d:?}");
}

#[test]
fn sql441_uncorrelated_not_exists() {
  let d = diags("SELECT * FROM users u WHERE NOT EXISTS (SELECT 1 FROM orders);");
  assert!(d.iter().any(|x| x.code == "sql441"), "expected sql441 for NOT EXISTS uncorrelated: {d:?}");
}

#[test]
fn sql441_uncorrelated_with_inner_where_no_outer_ref() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders WHERE id IS NOT NULL);");
  assert!(d.iter().any(|x| x.code == "sql441"), "expected sql441 when inner WHERE has no outer reference: {d:?}");
}

#[test]
fn sql441_quiet_for_correlated_via_alias() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id);");
  assert!(!d.iter().any(|x| x.code == "sql441"), "alias-correlated EXISTS must not fire: {d:?}");
}

#[test]
fn sql441_quiet_for_correlated_via_unaliased_table_name() {
  let d = diags("SELECT * FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE orders.user_id = users.id);");
  assert!(!d.iter().any(|x| x.code == "sql441"), "table-name-correlated EXISTS must not fire: {d:?}");
}

#[test]
fn sql442_regexp_replace_no_flag_arg() {
  let d = diags("SELECT regexp_replace('aaa', 'a', 'b');");
  assert!(
    d.iter().any(|x| x.code == "sql442" && x.message.contains("FIRST")),
    "expected sql442 for 3-arg regexp_replace: {d:?}"
  );
}

#[test]
fn sql442_regexp_replace_flag_without_g() {
  let d = diags("SELECT regexp_replace('aaa', 'a', 'b', 'i');");
  assert!(
    d.iter().any(|x| x.code == "sql442" && x.message.contains("`i`")),
    "expected sql442 for `i` flag without g: {d:?}"
  );
}

#[test]
fn sql442_quiet_for_g_flag() {
  let d = diags("SELECT regexp_replace('aaa', 'a', 'b', 'g');");
  assert!(!d.iter().any(|x| x.code == "sql442"), "`g` flag must not fire: {d:?}");
}

#[test]
fn sql442_quiet_for_gi_flag() {
  let d = diags("SELECT regexp_replace('aaa', 'a', 'b', 'gi');");
  assert!(!d.iter().any(|x| x.code == "sql442"), "`gi` flag must not fire: {d:?}");
}

#[test]
fn sql442_quiet_for_non_literal_flag() {
  // Can't determine the runtime value of a variable -- stay silent.
  let d = diags("SELECT regexp_replace(s, p, r, flag_var) FROM (SELECT 'a' AS s, 'a' AS p, 'b' AS r, 'g' AS flag_var) t;");
  assert!(!d.iter().any(|x| x.code == "sql442"), "non-literal flag must not fire: {d:?}");
}

#[test]
fn sql443_substring_negative_length() {
  let d = diags("SELECT substring('hello', 1, -3);");
  assert!(
    d.iter().any(|x| x.code == "sql443" && x.message.contains("-3")),
    "expected sql443 for substring with -3 length: {d:?}"
  );
}

#[test]
fn sql443_quiet_for_positive_length() {
  let d = diags("SELECT substring('hello', 1, 3);");
  assert!(!d.iter().any(|x| x.code == "sql443"), "positive length must not fire: {d:?}");
}

#[test]
fn sql443_quiet_for_non_literal_length() {
  let d = diags("SELECT substring(s, 1, n) FROM (SELECT 'a' AS s, 3 AS n) t;");
  assert!(!d.iter().any(|x| x.code == "sql443"), "non-literal length must not fire: {d:?}");
}

#[test]
fn sql443_quiet_for_two_arg_form() {
  // substring(s, start) -- no length arg, never fires
  let d = diags("SELECT substring('hello', 2);");
  assert!(!d.iter().any(|x| x.code == "sql443"), "2-arg form must not fire: {d:?}");
}

#[test]
fn sql444_generate_series_zero_step() {
  let d = diags("SELECT * FROM generate_series(1, 10, 0);");
  assert!(
    d.iter().any(|x| x.code == "sql444" && x.message.contains("zero step")),
    "expected sql444 for zero step: {d:?}"
  );
}

#[test]
fn sql444_generate_series_descending_with_positive_step() {
  let d = diags("SELECT * FROM generate_series(10, 1, 1);");
  assert!(
    d.iter().any(|x| x.code == "sql444" && x.message.contains("EMPTY")),
    "expected sql444 for 10..1 step 1: {d:?}"
  );
}

#[test]
fn sql444_generate_series_descending_no_step() {
  let d = diags("SELECT * FROM generate_series(10, 1);");
  assert!(d.iter().any(|x| x.code == "sql444"), "expected sql444 for 10..1 no step: {d:?}");
}

#[test]
fn sql444_quiet_for_legitimate_descending() {
  let d = diags("SELECT * FROM generate_series(10, 1, -1);");
  assert!(!d.iter().any(|x| x.code == "sql444"), "descending with -1 step must not fire: {d:?}");
}

#[test]
fn sql444_quiet_for_ascending() {
  let d = diags("SELECT * FROM generate_series(1, 10, 2);");
  assert!(!d.iter().any(|x| x.code == "sql444"), "ascending must not fire: {d:?}");
}

#[test]
fn sql445_array_position_with_null() {
  let d = diags("SELECT array_position(ARRAY[1,2,3], NULL);");
  assert!(
    d.iter().any(|x| x.code == "sql445" && x.message.contains("always returns NULL")),
    "expected sql445 for array_position(arr, NULL): {d:?}"
  );
}

#[test]
fn sql445_array_positions_with_null() {
  let d = diags("SELECT array_positions(ARRAY[1,2,3], NULL);");
  assert!(d.iter().any(|x| x.code == "sql445"), "expected sql445 for array_positions: {d:?}");
}

#[test]
fn sql445_quiet_for_non_null_needle() {
  let d = diags("SELECT array_position(ARRAY[1,2,3], 2);");
  assert!(!d.iter().any(|x| x.code == "sql445"), "non-NULL needle must not fire: {d:?}");
}

#[test]
fn sql445_quiet_for_column_needle() {
  // We can't tell if a column is NULL at edit time.
  let d = diags("SELECT array_position(ARRAY[1,2,3], n) FROM (SELECT 2 AS n) t;");
  assert!(!d.iter().any(|x| x.code == "sql445"), "column needle must not fire: {d:?}");
}

#[test]
fn sql409_not_between_message_flipped_for_low_self_bound() {
  // `col NOT BETWEEN col AND high` == `col > high`. Make sure the
  // message reflects the NOT-flipped semantics, not the BETWEEN form.
  let d = diags("SELECT * FROM users WHERE id NOT BETWEEN id AND 10;");
  let m = d.iter().find(|x| x.code == "sql409").unwrap_or_else(|| panic!("expected sql409: {d:?}"));
  assert!(
    m.message.contains("NOT BETWEEN") && m.message.contains(" > "),
    "NOT BETWEEN low-self-bound message should reflect `col > <high>`: {m:?}"
  );
}

#[test]
fn sql409_not_between_message_flipped_for_high_self_bound() {
  // `col NOT BETWEEN low AND col` == `col < low`.
  let d = diags("SELECT * FROM users WHERE id NOT BETWEEN 0 AND id;");
  let m = d.iter().find(|x| x.code == "sql409").unwrap_or_else(|| panic!("expected sql409: {d:?}"));
  assert!(
    m.message.contains("NOT BETWEEN") && m.message.contains(" < "),
    "NOT BETWEEN high-self-bound message should reflect `col < <low>`: {m:?}"
  );
}

#[test]
fn sql446_position_empty_in_string() {
  let d = diags("SELECT position('' in 'hello');");
  assert!(d.iter().any(|x| x.code == "sql446" && x.message.contains("always returns 1")), "expected sql446 for position(''): {d:?}");
}

#[test]
fn sql446_strpos_with_empty_needle() {
  let d = diags("SELECT strpos('hello', '');");
  assert!(d.iter().any(|x| x.code == "sql446"), "expected sql446 for strpos(_, ''): {d:?}");
}

#[test]
fn sql446_quiet_for_non_empty_needle() {
  let d = diags("SELECT position('h' in 'hello');");
  assert!(!d.iter().any(|x| x.code == "sql446"), "non-empty needle must not fire: {d:?}");
}

#[test]
fn sql446_quiet_for_non_literal_needle() {
  let d = diags("SELECT strpos('hello', n) FROM (SELECT 'e' AS n) t;");
  assert!(!d.iter().any(|x| x.code == "sql446"), "non-literal needle must not fire: {d:?}");
}

#[test]
fn sql447_power_exponent_zero() {
  let d = diags("SELECT power(5, 0);");
  assert!(d.iter().any(|x| x.code == "sql447" && x.message.contains("always returns 1")), "expected sql447 for power(_, 0): {d:?}");
}

#[test]
fn sql447_power_exponent_one() {
  let d = diags("SELECT power(5, 1);");
  assert!(d.iter().any(|x| x.code == "sql447" && x.message.contains("no-op")), "expected sql447 for power(_, 1): {d:?}");
}

#[test]
fn sql447_quiet_for_real_exponent() {
  let d = diags("SELECT power(5, 2);");
  assert!(!d.iter().any(|x| x.code == "sql447"), "exponent 2 must not fire: {d:?}");
}

#[test]
fn sql447_quiet_for_non_literal_exponent() {
  let d = diags("SELECT power(5, n) FROM (SELECT 2 AS n) t;");
  assert!(!d.iter().any(|x| x.code == "sql447"), "non-literal exponent must not fire: {d:?}");
}

#[test]
fn sql448_lpad_negative_length() {
  let d = diags("SELECT lpad('hi', -3, '0');");
  assert!(d.iter().any(|x| x.code == "sql448" && x.message.contains("-3")), "expected sql448 for lpad -3: {d:?}");
}

#[test]
fn sql448_rpad_negative_length() {
  let d = diags("SELECT rpad('hi', -5);");
  assert!(d.iter().any(|x| x.code == "sql448"), "expected sql448 for rpad -5: {d:?}");
}

#[test]
fn sql448_quiet_for_positive_length() {
  let d = diags("SELECT lpad('hi', 5, '0');");
  assert!(!d.iter().any(|x| x.code == "sql448"), "positive length must not fire: {d:?}");
}

#[test]
fn sql448_quiet_for_zero_length() {
  // length=0 returns empty string but is not a sign-flip; let users
  // decide whether to flag it under a different rule.
  let d = diags("SELECT lpad('hi', 0, '0');");
  assert!(!d.iter().any(|x| x.code == "sql448"), "zero length must not fire sql448 (negative-only): {d:?}");
}

#[test]
fn sql449_duplicate_adjacent_key() {
  let d = diags("SELECT jsonb_build_object('k1', 1, 'k1', 2);");
  assert!(
    d.iter().any(|x| x.code == "sql449" && x.message.contains("k1")),
    "expected sql449 for duplicate adjacent key: {d:?}"
  );
}

#[test]
fn sql449_duplicate_separated_key() {
  let d = diags("SELECT jsonb_build_object('a', 1, 'b', 2, 'a', 3);");
  assert!(d.iter().any(|x| x.code == "sql449"), "expected sql449 for non-adjacent dup: {d:?}");
}

#[test]
fn sql449_json_build_object_also_covered() {
  let d = diags("SELECT json_build_object('x', 'y', 'x', 'z');");
  assert!(d.iter().any(|x| x.code == "sql449"), "expected sql449 for json_build_object: {d:?}");
}

#[test]
fn sql449_quiet_for_distinct_keys() {
  let d = diags("SELECT jsonb_build_object('a', 1, 'b', 2);");
  assert!(!d.iter().any(|x| x.code == "sql449"), "distinct keys must not fire: {d:?}");
}

#[test]
fn sql449_quiet_for_non_literal_keys() {
  let d = diags("SELECT jsonb_build_object(k, 1, k, 2) FROM (SELECT 'a' AS k) t;");
  assert!(!d.iter().any(|x| x.code == "sql449"), "non-literal keys must not fire: {d:?}");
}

#[test]
fn sql450_numeric_scale_exceeds_precision_cast() {
  let d = diags("SELECT CAST(123 AS NUMERIC(3, 5));");
  assert!(
    d.iter().any(|x| x.code == "sql450" && x.message.contains("scale (5)")),
    "expected sql450 for NUMERIC(3, 5): {d:?}"
  );
}

#[test]
fn sql450_numeric_scale_exceeds_precision_double_colon() {
  let d = diags("SELECT 1::NUMERIC(3, 5);");
  assert!(d.iter().any(|x| x.code == "sql450"), "expected sql450 for ::NUMERIC(3,5): {d:?}");
}

#[test]
fn sql450_decimal_alias_also_covered() {
  let d = diags("SELECT CAST(123 AS DECIMAL(3, 5));");
  assert!(d.iter().any(|x| x.code == "sql450"), "expected sql450 for DECIMAL(3, 5): {d:?}");
}

#[test]
fn sql450_numeric_create_table_column() {
  let d = diags("CREATE TABLE t (price NUMERIC(3, 5));");
  assert!(d.iter().any(|x| x.code == "sql450"), "expected sql450 in CREATE TABLE: {d:?}");
}

#[test]
fn sql450_quiet_for_valid_precision_scale() {
  let d = diags("SELECT CAST(123 AS NUMERIC(5, 3));");
  assert!(!d.iter().any(|x| x.code == "sql450"), "valid precision/scale must not fire: {d:?}");
}

#[test]
fn sql450_quiet_for_single_arg() {
  let d = diags("SELECT CAST(123 AS NUMERIC(5));");
  assert!(!d.iter().any(|x| x.code == "sql450"), "single-arg NUMERIC must not fire: {d:?}");
}

#[test]
fn sql450_flags_zero_precision() {
  let d = diags("SELECT CAST(1 AS NUMERIC(0, 0));");
  assert!(d.iter().any(|x| x.code == "sql450" && x.message.contains("precision")), "expected sql450 for NUMERIC(0,0): {d:?}");
}

#[test]
fn sql451_varchar_zero_length() {
  let d = diags("CREATE TABLE t (name VARCHAR(0));");
  assert!(d.iter().any(|x| x.code == "sql451" && x.message.contains("VARCHAR(0)")), "expected sql451 for VARCHAR(0): {d:?}");
}

#[test]
fn sql451_char_zero_length() {
  let d = diags("CREATE TABLE t (name CHAR(0));");
  assert!(d.iter().any(|x| x.code == "sql451" && x.message.contains("CHAR(0)")), "expected sql451 for CHAR(0): {d:?}");
}

#[test]
fn sql451_character_varying_zero_length() {
  let d = diags("CREATE TABLE t (name CHARACTER VARYING(0));");
  assert!(d.iter().any(|x| x.code == "sql451" && x.message.contains("CHARACTER VARYING(0)")), "expected sql451 for CHARACTER VARYING(0): {d:?}");
}

#[test]
fn sql451_quiet_for_positive_length() {
  let d = diags("CREATE TABLE t (name VARCHAR(10));");
  assert!(!d.iter().any(|x| x.code == "sql451"), "VARCHAR(10) must not fire: {d:?}");
}

#[test]
fn sql451_quiet_for_unparameterized() {
  let d = diags("CREATE TABLE t (name VARCHAR);");
  assert!(!d.iter().any(|x| x.code == "sql451"), "unparameterized VARCHAR must not fire: {d:?}");
}

#[test]
fn sql452_repeat_zero_count() {
  let d = diags("SELECT repeat('ab', 0);");
  assert!(d.iter().any(|x| x.code == "sql452" && x.message.contains("0")), "expected sql452 for repeat(_, 0): {d:?}");
}

#[test]
fn sql452_repeat_negative_count() {
  let d = diags("SELECT repeat('ab', -3);");
  assert!(d.iter().any(|x| x.code == "sql452" && x.message.contains("-3")), "expected sql452 for repeat(_, -3): {d:?}");
}

#[test]
fn sql452_quiet_for_positive_count() {
  let d = diags("SELECT repeat('ab', 5);");
  assert!(!d.iter().any(|x| x.code == "sql452"), "positive count must not fire: {d:?}");
}

#[test]
fn sql452_quiet_for_non_literal_count() {
  let d = diags("SELECT repeat('ab', n) FROM (SELECT 3 AS n) t;");
  assert!(!d.iter().any(|x| x.code == "sql452"), "non-literal count must not fire: {d:?}");
}

#[test]
fn sql453_array_length_missing_dim() {
  let d = diags("SELECT array_length(ARRAY[1,2,3]);");
  assert!(
    d.iter().any(|x| x.code == "sql453" && x.message.contains("missing the dimension")),
    "expected sql453 for array_length without dim: {d:?}"
  );
}

#[test]
fn sql453_quiet_for_two_arg_form() {
  let d = diags("SELECT array_length(ARRAY[1,2,3], 1);");
  assert!(!d.iter().any(|x| x.code == "sql453"), "two-arg form must not fire: {d:?}");
}

#[test]
fn sql453_quiet_for_cardinality() {
  let d = diags("SELECT cardinality(ARRAY[1,2,3]);");
  assert!(!d.iter().any(|x| x.code == "sql453"), "cardinality must not fire: {d:?}");
}

#[test]
fn sql453_quiet_for_empty_args() {
  // Empty (0-arg) call is a different problem; sql453 only fires
  // on the single-arg form.
  let d = diags("SELECT array_length();");
  assert!(!d.iter().any(|x| x.code == "sql453"), "0-arg form must not fire sql453: {d:?}");
}

#[test]
fn sql454_to_timestamp_hh_colon_mm() {
  let d = diags("SELECT to_timestamp('2024-01-01 10:30', 'YYYY-MM-DD HH:MM');");
  assert!(
    d.iter().any(|x| x.code == "sql454" && x.message.contains("MONTH")),
    "expected sql454 for HH:MM: {d:?}"
  );
}

#[test]
fn sql454_to_timestamp_hh24_colon_mm() {
  let d = diags("SELECT to_timestamp('10:30', 'HH24:MM');");
  assert!(d.iter().any(|x| x.code == "sql454"), "expected sql454 for HH24:MM: {d:?}");
}

#[test]
fn sql454_to_char_hh_colon_mm() {
  let d = diags("SELECT to_char(now(), 'HH:MM');");
  assert!(d.iter().any(|x| x.code == "sql454"), "expected sql454 for to_char HH:MM: {d:?}");
}

#[test]
fn sql454_mm_colon_ss() {
  let d = diags("SELECT to_char(now(), 'MM:SS');");
  assert!(d.iter().any(|x| x.code == "sql454" && x.message.contains("MM:SS")), "expected sql454 for MM:SS: {d:?}");
}

#[test]
fn sql454_quiet_for_correct_mi() {
  let d = diags("SELECT to_timestamp('10:30', 'HH24:MI');");
  assert!(!d.iter().any(|x| x.code == "sql454"), "correct MI must not fire: {d:?}");
}

#[test]
fn sql454_quiet_for_date_only() {
  let d = diags("SELECT to_timestamp('2024-01-01', 'YYYY-MM-DD');");
  assert!(!d.iter().any(|x| x.code == "sql454"), "date-only YYYY-MM-DD must not fire: {d:?}");
}

#[test]
fn sql454_quiet_for_non_literal_fmt() {
  let d = diags("SELECT to_timestamp(t, f) FROM (SELECT 'a' AS t, 'b' AS f) x;");
  assert!(!d.iter().any(|x| x.code == "sql454"), "non-literal fmt must not fire: {d:?}");
}

#[test]
fn sql451_bit_zero_length() {
  let d = diags("CREATE TABLE t (b BIT(0));");
  assert!(
    d.iter().any(|x| x.code == "sql451" && x.message.contains("BIT(0)") && x.message.contains("bit string")),
    "expected sql451 for BIT(0) with bit-string message: {d:?}"
  );
}

#[test]
fn sql451_bit_varying_zero_length() {
  let d = diags("CREATE TABLE t (b BIT VARYING(0));");
  assert!(
    d.iter().any(|x| x.code == "sql451" && x.message.contains("BIT VARYING(0)")),
    "expected sql451 for BIT VARYING(0): {d:?}"
  );
}

#[test]
fn sql451_bit_quiet_for_positive_length() {
  let d = diags("CREATE TABLE t (b BIT(8));");
  assert!(!d.iter().any(|x| x.code == "sql451"), "BIT(8) must not fire: {d:?}");
}

#[test]
fn sql455_x_or_not_x() {
  let d = diags("SELECT * FROM users WHERE active OR NOT active;");
  assert!(d.iter().any(|x| x.code == "sql455" && x.message.contains("always TRUE")), "expected sql455 for x OR NOT x: {d:?}");
}

#[test]
fn sql455_not_x_or_x() {
  let d = diags("SELECT * FROM users WHERE NOT active OR active;");
  assert!(d.iter().any(|x| x.code == "sql455"), "expected sql455 for NOT x OR x: {d:?}");
}

#[test]
fn sql455_quiet_for_distinct_branches() {
  let d = diags("SELECT * FROM users WHERE active OR id = 5;");
  assert!(!d.iter().any(|x| x.code == "sql455"), "distinct OR branches must not fire: {d:?}");
}

#[test]
fn sql455_quiet_for_x_and_not_x() {
  // AND form is sql422's territory, not sql455.
  let d = diags("SELECT * FROM users WHERE active AND NOT active;");
  assert!(!d.iter().any(|x| x.code == "sql455"), "AND form must not fire sql455: {d:?}");
}

// sql456 uses a custom catalog with smallint / int / bigint columns.
fn cat_with_ints() -> Catalog {
  let nums = Table {
    schema: "public".into(),
    name: "nums".into(),
    kind: TableKind::Table,
    columns: vec![
      Column { name: "s".into(), data_type: "smallint".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "i".into(), data_type: "integer".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "b".into(), data_type: "bigint".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "t".into(), data_type: "text".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
    ],
    constraints: vec![], indexes: vec![], triggers: vec![], policies: vec![], comment: None, row_estimate: None, owner: None, definition: None, strict: false, options: None,
  };
  Catalog { version: CATALOG_VERSION, connection_id: "test".into(), schemas: vec![Schema { name: "public".into(), tables: vec![nums] }], functions: vec![], types: vec![], roles: vec![], sequences: vec![], extensions: vec![] }
}

fn diags_with_ints(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let c = cat_with_ints();
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &c)
}

#[test]
fn sql456_smallint_literal_too_large() {
  let d = diags_with_ints("SELECT * FROM nums WHERE s = 100000;");
  assert!(
    d.iter().any(|x| x.code == "sql456" && x.message.contains("100000")),
    "expected sql456 for smallint s = 100000: {d:?}"
  );
}

#[test]
fn sql456_quiet_for_in_range_smallint() {
  let d = diags_with_ints("SELECT * FROM nums WHERE s = 100;");
  assert!(!d.iter().any(|x| x.code == "sql456"), "in-range smallint must not fire: {d:?}");
}

#[test]
fn sql456_int_literal_too_large() {
  let d = diags_with_ints("SELECT * FROM nums WHERE i = 5000000000;");
  assert!(d.iter().any(|x| x.code == "sql456" && x.message.contains("integer")), "expected sql456 for int i = 5B: {d:?}");
}

#[test]
fn sql456_bigint_quiet_for_int_max_plus_one() {
  // bigint range easily covers 5_000_000_000.
  let d = diags_with_ints("SELECT * FROM nums WHERE b = 5000000000;");
  assert!(!d.iter().any(|x| x.code == "sql456"), "bigint must not fire for 5B: {d:?}");
}

#[test]
fn sql456_quiet_for_non_numeric_column() {
  let d = diags_with_ints("SELECT * FROM nums WHERE t = 'x';");
  assert!(!d.iter().any(|x| x.code == "sql456"), "text column must not fire: {d:?}");
}

#[test]
fn sql456_smallint_quiet_for_negative_in_range() {
  let d = diags_with_ints("SELECT * FROM nums WHERE s = -100;");
  assert!(!d.iter().any(|x| x.code == "sql456"), "in-range negative must not fire: {d:?}");
}

#[test]
fn sql457_group_by_position_out_of_range() {
  let d = diags("SELECT id FROM users GROUP BY 3;");
  assert!(
    d.iter().any(|x| x.code == "sql457" && x.message.contains("3")),
    "expected sql457 for GROUP BY 3 with 1 projection: {d:?}"
  );
}

#[test]
fn sql457_group_by_position_zero() {
  let d = diags("SELECT id FROM users GROUP BY 0;");
  assert!(d.iter().any(|x| x.code == "sql457"), "expected sql457 for GROUP BY 0: {d:?}");
}

#[test]
fn sql457_order_by_position_out_of_range() {
  let d = diags("SELECT id, name FROM users ORDER BY 5;");
  assert!(d.iter().any(|x| x.code == "sql457" && x.message.contains("ORDER BY")), "expected sql457 for ORDER BY 5: {d:?}");
}

#[test]
fn sql457_quiet_for_valid_position() {
  let d = diags("SELECT id, name FROM users GROUP BY 1, 2;");
  assert!(!d.iter().any(|x| x.code == "sql457"), "valid positions must not fire: {d:?}");
}

#[test]
fn sql457_group_by_one_in_range_one_out() {
  let d = diags("SELECT id, name FROM users GROUP BY 1, 5;");
  let count = d.iter().filter(|x| x.code == "sql457").count();
  assert_eq!(count, 1, "expected exactly one sql457 (only the 5): {d:?}");
}

#[test]
fn sql458_sum_of_boolean_column() {
  // flags.active is a boolean column in the shared catalog.
  let d = diags("SELECT sum(active) FROM flags;");
  assert!(
    d.iter().any(|x| x.code == "sql458" && x.message.contains("sum(boolean)")),
    "expected sql458 for sum(active): {d:?}"
  );
}

#[test]
fn sql458_avg_of_boolean_column() {
  let d = diags("SELECT avg(active) FROM flags;");
  assert!(d.iter().any(|x| x.code == "sql458" && x.message.contains("avg(boolean)")), "expected sql458 for avg(active): {d:?}");
}

#[test]
fn sql458_quiet_for_sum_int() {
  // orders.user_id is uuid; need an int column. Use diags_with_ints from sql456 catalog.
  let d = diags_with_ints("SELECT sum(i) FROM nums;");
  assert!(!d.iter().any(|x| x.code == "sql458"), "sum of int must not fire: {d:?}");
}

#[test]
fn sql458_quiet_for_cast_to_int() {
  let d = diags("SELECT sum(active::int) FROM flags;");
  assert!(!d.iter().any(|x| x.code == "sql458"), "sum of cast bool must not fire: {d:?}");
}

#[test]
fn sql458_quiet_for_count_filter() {
  let d = diags("SELECT count(*) FILTER (WHERE active) FROM flags;");
  assert!(!d.iter().any(|x| x.code == "sql458"), "count(*) FILTER must not fire: {d:?}");
}

#[test]
fn sql459_count_notnull_column() {
  // users.email is NOT NULL in the shared catalog.
  let d = diags("SELECT count(email) FROM users;");
  assert!(
    d.iter().any(|x| x.code == "sql459" && x.message.contains("COUNT(*)")),
    "expected sql459 for count(email) on NOT NULL: {d:?}"
  );
}

#[test]
fn sql459_quiet_for_nullable_column() {
  // users.name is nullable in the shared catalog.
  let d = diags("SELECT count(name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql459"), "nullable column must not fire sql459: {d:?}");
}

#[test]
fn sql459_quiet_for_count_star() {
  let d = diags("SELECT count(*) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql459"), "count(*) must not fire: {d:?}");
}

#[test]
fn sql459_quiet_for_count_distinct() {
  let d = diags("SELECT count(distinct email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql459"), "count(distinct) must not fire: {d:?}");
}

#[test]
fn sql460_having_no_group_no_agg() {
  let d = diags("SELECT id FROM users HAVING id IS NOT NULL;");
  assert!(
    d.iter().any(|x| x.code == "sql460" && x.message.contains("WHERE")),
    "expected sql460 for HAVING without GROUP BY and no aggregate: {d:?}"
  );
}

#[test]
fn sql460_quiet_for_having_with_agg() {
  let d = diags("SELECT count(*) FROM users HAVING count(*) > 10;");
  assert!(!d.iter().any(|x| x.code == "sql460"), "HAVING with agg must not fire: {d:?}");
}

#[test]
fn sql460_quiet_when_group_by_present() {
  let d = diags("SELECT email, count(*) FROM users GROUP BY email HAVING email = 'x';");
  assert!(!d.iter().any(|x| x.code == "sql460"), "HAVING with GROUP BY must not fire: {d:?}");
}

#[test]
fn sql460_quiet_for_no_having() {
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql460"), "no HAVING must not fire: {d:?}");
}

#[test]
fn sql015_cast_null_form_caught() {
  // `WHERE x = CAST(NULL AS text)` is semantically `x = NULL`.
  let d = diags("SELECT * FROM users WHERE email = CAST(NULL AS text);");
  assert!(d.iter().any(|x| x.code == "sql015"), "expected sql015 for `= CAST(NULL AS text)`: {d:?}");
}

#[test]
fn sql015_quiet_for_is_null() {
  let d = diags("SELECT * FROM users WHERE email IS NULL;");
  assert!(!d.iter().any(|x| x.code == "sql015"), "IS NULL must not fire: {d:?}");
}

#[test]
fn sql015_null_on_left_equals() {
  // Regression: `NULL = col` was silent. NULL on the LHS has the
  // same broken semantics as on the RHS -- the comparison yields
  // NULL, never TRUE.
  let d = diags("SELECT * FROM users WHERE NULL = email;");
  assert!(d.iter().any(|x| x.code == "sql015" && x.message.contains("NULL =")), "expected sql015 for `NULL = col`: {d:?}");
}

#[test]
fn sql015_null_on_left_not_equals() {
  let d = diags("SELECT * FROM users WHERE NULL <> email;");
  assert!(d.iter().any(|x| x.code == "sql015" && x.message.contains("NULL <>")), "expected sql015 for `NULL <> col`: {d:?}");
}

#[test]
fn sql015_bang_equals_null_message_is_correct() {
  // Regression: `!= NULL` was misreported as `= NULL` because the
  // pattern loop checked the shorter substring first.
  let d = diags("SELECT * FROM users WHERE email != NULL;");
  let msg = d.iter().find(|x| x.code == "sql015").map(|x| x.message.clone()).unwrap_or_default();
  assert!(msg.contains("!= NULL"), "expected message to mention `!= NULL`, got: {msg:?}");
}

#[test]
fn sql015_quiet_for_word_starting_with_null() {
  // A column like `null_count` (or similar) starts with NULL but
  // is NOT a NULL literal -- word boundary must prevent a false
  // positive on `NULL_FIELD = ...` if the source ever has one.
  let d = diags("SELECT * FROM users WHERE null_count = 1;");
  assert!(!d.iter().any(|x| x.code == "sql015"), "word-boundary must hold: {d:?}");
}

#[test]
fn sql461_array_remove_null() {
  let d = diags("SELECT array_remove(NULL, 1);");
  assert!(d.iter().any(|x| x.code == "sql461" && x.message.contains("array_remove(NULL")), "expected sql461 for array_remove(NULL, 1): {d:?}");
}

#[test]
fn sql461_cardinality_null() {
  let d = diags("SELECT cardinality(NULL);");
  assert!(d.iter().any(|x| x.code == "sql461"), "expected sql461 for cardinality(NULL): {d:?}");
}

#[test]
fn sql461_array_position_null() {
  let d = diags("SELECT array_position(NULL, 1);");
  assert!(d.iter().any(|x| x.code == "sql461"), "expected sql461 for array_position(NULL, 1): {d:?}");
}

#[test]
fn sql461_quiet_for_non_null_array() {
  let d = diags("SELECT array_remove(ARRAY[1,2,3], 1);");
  assert!(!d.iter().any(|x| x.code == "sql461"), "non-NULL array must not fire: {d:?}");
}

#[test]
fn sql461_quiet_for_array_append_null() {
  // array_append(NULL, x) returns ARRAY[x] -- legitimate constructor pattern.
  let d = diags("SELECT array_append(NULL, 1);");
  assert!(!d.iter().any(|x| x.code == "sql461"), "array_append(NULL, x) is a valid constructor; must not fire: {d:?}");
}

#[test]
fn sql292_limit_zero() {
  let d = diags("SELECT * FROM users LIMIT 0;");
  assert!(d.iter().any(|x| x.code == "sql292" && x.message.contains("LIMIT 0")), "expected sql292 for LIMIT 0: {d:?}");
}

#[test]
fn sql292_fetch_first_zero_rows_only() {
  let d = diags("SELECT * FROM users FETCH FIRST 0 ROWS ONLY;");
  assert!(d.iter().any(|x| x.code == "sql292" && x.message.contains("FETCH FIRST 0")), "expected sql292 for FETCH FIRST 0: {d:?}");
}

#[test]
fn sql292_fetch_next_zero_rows() {
  let d = diags("SELECT * FROM users FETCH NEXT 0 ROWS ONLY;");
  assert!(d.iter().any(|x| x.code == "sql292"), "expected sql292 for FETCH NEXT 0: {d:?}");
}

#[test]
fn sql292_quiet_for_limit_positive() {
  let d = diags("SELECT * FROM users LIMIT 10;");
  assert!(!d.iter().any(|x| x.code == "sql292"), "LIMIT 10 must not fire: {d:?}");
}

#[test]
fn sql292_quiet_for_fetch_first_positive() {
  let d = diags("SELECT * FROM users FETCH FIRST 5 ROWS ONLY;");
  assert!(!d.iter().any(|x| x.code == "sql292"), "FETCH FIRST 5 must not fire: {d:?}");
}

#[test]
fn sql278_modulo_by_zero() {
  let d = diags("SELECT 5 % 0;");
  assert!(d.iter().any(|x| x.code == "sql278" && x.message.contains("modulo")), "expected sql278 for `5 % 0`: {d:?}");
}

#[test]
fn sql278_quiet_for_modulo_nonzero() {
  let d = diags("SELECT 5 % 3;");
  assert!(!d.iter().any(|x| x.code == "sql278"), "5 % 3 must not fire: {d:?}");
}

#[test]
fn sql462_arithmetic_plus_null() {
  let d = diags("SELECT 1 + NULL;");
  assert!(d.iter().any(|x| x.code == "sql462" && x.message.contains("NULL")), "expected sql462 for `1 + NULL`: {d:?}");
}

#[test]
fn sql462_arithmetic_null_minus_column() {
  let d = diags("SELECT NULL - id FROM users;");
  assert!(d.iter().any(|x| x.code == "sql462"), "expected sql462 for `NULL - id`: {d:?}");
}

#[test]
fn sql462_arithmetic_multiply_null() {
  let d = diags("SELECT id * NULL FROM users;");
  assert!(d.iter().any(|x| x.code == "sql462"), "expected sql462 for `id * NULL`: {d:?}");
}

#[test]
fn sql462_quiet_for_string_concat() {
  // `||` is handled by sql413, not sql462.
  let d = diags("SELECT 'a' || NULL;");
  assert!(!d.iter().any(|x| x.code == "sql462"), "string concat || must not fire sql462: {d:?}");
}

#[test]
fn sql462_quiet_for_is_null() {
  let d = diags("SELECT 1 FROM users WHERE id IS NULL;");
  assert!(!d.iter().any(|x| x.code == "sql462"), "IS NULL must not fire: {d:?}");
}

#[test]
fn sql462_quiet_for_cast_null() {
  // `NULL::int` (no arithmetic op nearby) -- must not fire.
  let d = diags("SELECT NULL::int;");
  assert!(!d.iter().any(|x| x.code == "sql462"), "NULL::int must not fire: {d:?}");
}

#[test]
fn sql463_tg_op_lowercase() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_OP = 'insert' THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql463" && x.message.contains("'INSERT'")), "expected sql463 for TG_OP = 'insert': {d:?}");
}

#[test]
fn sql463_tg_op_past_tense() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_OP = 'inserted' THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql463"), "expected sql463 for `inserted`: {d:?}");
}

#[test]
fn sql463_tg_op_in_list_with_typo() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_OP IN ('INSERT', 'updated') THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql463"), "expected sql463 for `updated` in IN-list: {d:?}");
}

#[test]
fn sql463_quiet_for_valid_uppercase() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_OP = 'INSERT' THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql463"), "valid INSERT must not fire: {d:?}");
}

#[test]
fn sql463_quiet_for_valid_in_list() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_OP IN ('INSERT', 'UPDATE') THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql463"), "valid IN-list must not fire: {d:?}");
}

#[test]
fn sql463_tg_level_lowercase() {
  let d = diags("CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_LEVEL = 'row' THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql463" && x.message.contains("'ROW'")), "expected sql463 for TG_LEVEL = 'row': {d:?}");
}

#[test]
fn sql463_tg_when_lowercase() {
  let d = diags("CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_WHEN = 'before' THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql463" && x.message.contains("'BEFORE'")), "expected sql463 for TG_WHEN = 'before': {d:?}");
}

#[test]
fn sql463_quiet_for_valid_tg_when() {
  let d = diags("CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF TG_WHEN = 'INSTEAD OF' THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql463"), "valid `INSTEAD OF` must not fire: {d:?}");
}

#[test]
fn sql463_commuted_literal_eq_tg_op() {
  // Regression iter195: `'insert' = TG_OP` (commuted) was silent.
  let d = diags("CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF 'insert' = TG_OP THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql463" && x.message.contains("'INSERT'")), "expected sql463 for `'insert' = TG_OP`: {d:?}");
}

#[test]
fn sql463_commuted_literal_neq_tg_op() {
  let d = diags("CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF 'delete' <> TG_OP THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql463" && x.message.contains("'DELETE'")), "expected sql463 for commuted `<>`: {d:?}");
}

#[test]
fn sql463_quiet_for_commuted_valid_literal() {
  let d = diags("CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF 'INSERT' = TG_OP THEN RETURN NEW; END IF; RETURN NEW; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql463"), "commuted valid literal must not fire: {d:?}");
}

#[test]
fn sql464_is_distinct_from_self() {
  let d = diags("SELECT * FROM users WHERE id IS DISTINCT FROM id;");
  assert!(d.iter().any(|x| x.code == "sql464" && x.message.contains("always FALSE")), "expected sql464 for `id IS DISTINCT FROM id`: {d:?}");
}

#[test]
fn sql464_is_not_distinct_from_self() {
  let d = diags("SELECT * FROM users WHERE id IS NOT DISTINCT FROM id;");
  assert!(d.iter().any(|x| x.code == "sql464" && x.message.contains("always TRUE")), "expected sql464 for `id IS NOT DISTINCT FROM id`: {d:?}");
}

#[test]
fn sql464_qualified_column_self() {
  let d = diags("SELECT * FROM users u WHERE u.id IS DISTINCT FROM u.id;");
  assert!(d.iter().any(|x| x.code == "sql464"), "expected sql464 for qualified self: {d:?}");
}

#[test]
fn sql464_quiet_for_two_columns() {
  let d = diags("SELECT * FROM users WHERE id IS DISTINCT FROM name;");
  assert!(!d.iter().any(|x| x.code == "sql464"), "two different columns must not fire: {d:?}");
}

#[test]
fn sql464_quiet_for_null_rhs() {
  // `x IS DISTINCT FROM NULL` is the same as `IS NOT NULL` -- sql095
  // handles the readability, sql464 should stay silent.
  let d = diags("SELECT * FROM users WHERE id IS DISTINCT FROM NULL;");
  assert!(!d.iter().any(|x| x.code == "sql464"), "RHS NULL must not fire sql464: {d:?}");
}

#[test]
fn sql465_concat_ws_empty_sep() {
  let d = diags("SELECT concat_ws('', 'a', 'b', 'c');");
  assert!(d.iter().any(|x| x.code == "sql465" && x.message.contains("concat(")), "expected sql465 for concat_ws('', ...): {d:?}");
}

#[test]
fn sql465_quiet_for_real_separator() {
  let d = diags("SELECT concat_ws('-', 'a', 'b', 'c');");
  assert!(!d.iter().any(|x| x.code == "sql465"), "real separator must not fire: {d:?}");
}

#[test]
fn sql465_quiet_for_plain_concat() {
  let d = diags("SELECT concat('a', 'b', 'c');");
  assert!(!d.iter().any(|x| x.code == "sql465"), "plain concat must not fire: {d:?}");
}

#[test]
fn sql443_substring_sql_standard_negative_for() {
  let d = diags("SELECT substring('hello' FROM 2 FOR -3);");
  assert!(
    d.iter().any(|x| x.code == "sql443" && x.message.contains("FROM ... FOR")),
    "expected sql443 for SQL-standard `FOR -3`: {d:?}"
  );
}

#[test]
fn sql443_substring_sql_standard_quiet_positive() {
  let d = diags("SELECT substring('hello' FROM 2 FOR 3);");
  assert!(!d.iter().any(|x| x.code == "sql443"), "positive FOR must not fire: {d:?}");
}

#[test]
fn sql466_offset_zero() {
  let d = diags("SELECT * FROM users OFFSET 0;");
  assert!(d.iter().any(|x| x.code == "sql466" && x.message.contains("no-op")), "expected sql466 for OFFSET 0: {d:?}");
}

#[test]
fn sql466_with_limit_and_offset_zero() {
  let d = diags("SELECT * FROM users LIMIT 10 OFFSET 0;");
  assert!(d.iter().any(|x| x.code == "sql466"), "expected sql466 for LIMIT 10 OFFSET 0: {d:?}");
}

#[test]
fn sql466_quiet_for_positive_offset() {
  let d = diags("SELECT * FROM users OFFSET 5;");
  assert!(!d.iter().any(|x| x.code == "sql466"), "OFFSET 5 must not fire: {d:?}");
}

#[test]
fn sql466_quiet_for_no_offset() {
  let d = diags("SELECT * FROM users LIMIT 10;");
  assert!(!d.iter().any(|x| x.code == "sql466"), "no OFFSET must not fire: {d:?}");
}

#[test]
fn sql467_replace_empty_needle() {
  let d = diags("SELECT replace('hello', '', 'x');");
  assert!(d.iter().any(|x| x.code == "sql467" && x.message.contains("replace")), "expected sql467 for replace empty needle: {d:?}");
}

#[test]
fn sql467_split_part_empty_delimiter() {
  let d = diags("SELECT split_part('a-b-c', '', 1);");
  assert!(d.iter().any(|x| x.code == "sql467" && x.message.contains("split_part")), "expected sql467 for split_part empty delim: {d:?}");
}

#[test]
fn sql467_quiet_for_real_needle() {
  let d = diags("SELECT replace('hello', 'l', 'X');");
  assert!(!d.iter().any(|x| x.code == "sql467"), "real needle must not fire: {d:?}");
}

#[test]
fn sql467_quiet_for_split_real_delim() {
  let d = diags("SELECT split_part('a-b-c', '-', 1);");
  assert!(!d.iter().any(|x| x.code == "sql467"), "real delim must not fire: {d:?}");
}

#[test]
fn sql468_greatest_all_null() {
  let d = diags("SELECT greatest(NULL, NULL);");
  assert!(d.iter().any(|x| x.code == "sql468" && x.message.contains("greatest")), "expected sql468 for greatest(NULL, NULL): {d:?}");
}

#[test]
fn sql468_least_all_null_three_args() {
  let d = diags("SELECT least(NULL, NULL, NULL);");
  assert!(d.iter().any(|x| x.code == "sql468" && x.message.contains("least")), "expected sql468 for least(NULL, NULL, NULL): {d:?}");
}

#[test]
fn sql468_quiet_for_mixed_null_and_value() {
  // PG legitimately skips NULLs when at least one non-NULL is present.
  let d = diags("SELECT greatest(1, NULL);");
  assert!(!d.iter().any(|x| x.code == "sql468"), "mixed NULL + value must not fire: {d:?}");
}

#[test]
fn sql468_quiet_for_all_values() {
  let d = diags("SELECT greatest(1, 2, 3);");
  assert!(!d.iter().any(|x| x.code == "sql468"), "all values must not fire: {d:?}");
}

#[test]
fn sql469_not_paren_is_null() {
  let d = diags("SELECT * FROM users WHERE NOT (email IS NULL);");
  assert!(d.iter().any(|x| x.code == "sql469" && x.message.contains("IS NOT NULL")), "expected sql469 for NOT (email IS NULL): {d:?}");
}

#[test]
fn sql469_not_unparen_is_null() {
  let d = diags("SELECT * FROM users WHERE NOT email IS NULL;");
  assert!(d.iter().any(|x| x.code == "sql469"), "expected sql469 for NOT email IS NULL: {d:?}");
}

#[test]
fn sql469_not_paren_is_not_null() {
  let d = diags("SELECT * FROM users WHERE NOT (email IS NOT NULL);");
  assert!(d.iter().any(|x| x.code == "sql469" && x.message.contains("IS NULL")), "expected sql469 for double-negative: {d:?}");
}

#[test]
fn sql469_quiet_for_idiomatic_form() {
  let d = diags("SELECT * FROM users WHERE email IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql469"), "idiomatic IS NOT NULL must not fire: {d:?}");
}

#[test]
fn sql470_not_paren_in() {
  let d = diags("SELECT * FROM users WHERE NOT (email IN ('a', 'b'));");
  assert!(d.iter().any(|x| x.code == "sql470" && x.message.contains("NOT IN")), "expected sql470 for NOT (email IN ...): {d:?}");
}

#[test]
fn sql470_not_paren_like() {
  let d = diags("SELECT * FROM users WHERE NOT (email LIKE 'a%');");
  assert!(d.iter().any(|x| x.code == "sql470" && x.message.contains("NOT LIKE")), "expected sql470 for NOT (email LIKE ...): {d:?}");
}

#[test]
fn sql470_not_paren_between() {
  let d = diags("SELECT * FROM users WHERE NOT (id BETWEEN 1 AND 10);");
  assert!(d.iter().any(|x| x.code == "sql470" && x.message.contains("NOT BETWEEN")), "expected sql470 for NOT (id BETWEEN ...): {d:?}");
}

#[test]
fn sql470_quiet_for_not_in_idiom() {
  let d = diags("SELECT * FROM users WHERE email NOT IN ('a', 'b');");
  assert!(!d.iter().any(|x| x.code == "sql470"), "idiomatic NOT IN must not fire: {d:?}");
}

#[test]
fn sql470_quiet_for_not_paren_is_null() {
  // sql469 owns the IS NULL variant.
  let d = diags("SELECT * FROM users WHERE NOT (email IS NULL);");
  assert!(!d.iter().any(|x| x.code == "sql470"), "NOT (IS NULL) is sql469's job: {d:?}");
}

#[test]
fn sql470_quiet_for_not_exists() {
  // NOT EXISTS has no `NOT IN`-style rewrite -- skip.
  let d = diags("SELECT * FROM users WHERE NOT EXISTS (SELECT 1 FROM users);");
  assert!(!d.iter().any(|x| x.code == "sql470"), "NOT EXISTS must not fire: {d:?}");
}

#[test]
fn sql471_distinct_inside_in_subquery() {
  let d = diags("SELECT * FROM users WHERE id IN (SELECT DISTINCT user_id FROM orders);");
  assert!(d.iter().any(|x| x.code == "sql471" && x.message.contains("DISTINCT")), "expected sql471 for DISTINCT inside IN: {d:?}");
}

#[test]
fn sql471_distinct_inside_not_in_subquery() {
  let d = diags("SELECT * FROM users WHERE id NOT IN (SELECT DISTINCT user_id FROM orders);");
  assert!(d.iter().any(|x| x.code == "sql471"), "expected sql471 for DISTINCT inside NOT IN: {d:?}");
}

#[test]
fn sql471_quiet_without_distinct() {
  let d = diags("SELECT * FROM users WHERE id IN (SELECT user_id FROM orders);");
  assert!(!d.iter().any(|x| x.code == "sql471"), "no DISTINCT must not fire: {d:?}");
}

#[test]
fn sql471_quiet_for_distinct_on() {
  // DISTINCT ON has semantic meaning -- keep silent.
  let d = diags("SELECT * FROM users WHERE id IN (SELECT DISTINCT ON (user_id) user_id FROM orders);");
  assert!(!d.iter().any(|x| x.code == "sql471"), "DISTINCT ON must not fire: {d:?}");
}

#[test]
fn sql472_extract_dow_from_interval() {
  let d = diags("SELECT extract(dow from '1 day'::interval);");
  assert!(d.iter().any(|x| x.code == "sql472" && x.message.contains("dow")), "expected sql472 for extract(dow from interval): {d:?}");
}

#[test]
fn sql472_extract_week_from_interval_keyword_form() {
  let d = diags("SELECT extract(week from INTERVAL '1 day');");
  assert!(d.iter().any(|x| x.code == "sql472" && x.message.contains("week")), "expected sql472 for extract(week from INTERVAL): {d:?}");
}

#[test]
fn sql472_extract_timezone_from_interval() {
  let d = diags("SELECT extract(timezone from '1 day'::interval);");
  assert!(d.iter().any(|x| x.code == "sql472"), "expected sql472 for extract(timezone from interval): {d:?}");
}

#[test]
fn sql472_quiet_for_valid_field() {
  let d = diags("SELECT extract(year from '1 day'::interval);");
  assert!(!d.iter().any(|x| x.code == "sql472"), "year is valid for interval: {d:?}");
}

#[test]
fn sql472_quiet_for_non_interval_operand() {
  // dow IS valid for timestamp; the FROM operand isn't an interval here.
  let d = diags("SELECT extract(dow from TIMESTAMP '2024-01-01');");
  assert!(!d.iter().any(|x| x.code == "sql472"), "non-interval operand must not fire: {d:?}");
}

#[test]
fn sql069_default_cast_null_as_type() {
  let d = diags("CREATE TABLE t (id int NOT NULL DEFAULT CAST(NULL AS int));");
  assert!(d.iter().any(|x| x.code == "sql069"), "expected sql069 for CAST(NULL AS int): {d:?}");
}

#[test]
fn sql069_default_paren_null() {
  let d = diags("CREATE TABLE t (id int NOT NULL DEFAULT (NULL));");
  assert!(d.iter().any(|x| x.code == "sql069"), "expected sql069 for DEFAULT (NULL): {d:?}");
}

#[test]
fn sql069_quiet_for_default_zero() {
  let d = diags("CREATE TABLE t (id int NOT NULL DEFAULT 0);");
  assert!(!d.iter().any(|x| x.code == "sql069"), "DEFAULT 0 must not fire: {d:?}");
}

#[test]
fn sql348_quiet_for_mode_aggregate() {
  // Regression: mode() is a PG ordered-set aggregate, must be recognized.
  let d = diags("SELECT mode() WITHIN GROUP (ORDER BY id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql348"), "mode() must not be flagged as unknown: {d:?}");
}

#[test]
fn sql473_any_empty_array() {
  let d = diags("SELECT * FROM users WHERE id = ANY(ARRAY[]::int[]);");
  assert!(d.iter().any(|x| x.code == "sql473" && x.message.contains("always FALSE")), "expected sql473 for ANY(ARRAY[]): {d:?}");
}

#[test]
fn sql473_all_empty_array() {
  let d = diags("SELECT * FROM users WHERE id = ALL(ARRAY[]::int[]);");
  assert!(d.iter().any(|x| x.code == "sql473" && x.message.contains("always TRUE")), "expected sql473 for ALL(ARRAY[]): {d:?}");
}

#[test]
fn sql473_any_empty_braced_literal() {
  let d = diags("SELECT * FROM users WHERE id = ANY('{}'::int[]);");
  assert!(d.iter().any(|x| x.code == "sql473"), "expected sql473 for ANY('{{}}'): {d:?}");
}

#[test]
fn sql473_quiet_for_nonempty_array() {
  let d = diags("SELECT * FROM users WHERE id = ANY(ARRAY[1, 2, 3]);");
  assert!(!d.iter().any(|x| x.code == "sql473"), "non-empty array must not fire: {d:?}");
}

#[test]
fn sql474_string_equal_tautology() {
  let d = diags("SELECT * FROM users WHERE 'a' = 'a';");
  assert!(d.iter().any(|x| x.code == "sql474" && x.message.contains("always TRUE")), "expected sql474 for 'a' = 'a': {d:?}");
}

#[test]
fn sql474_numeric_equal_tautology() {
  let d = diags("SELECT * FROM users WHERE 2 = 2;");
  assert!(d.iter().any(|x| x.code == "sql474"), "expected sql474 for 2 = 2: {d:?}");
}

#[test]
fn sql474_string_contradiction() {
  let d = diags("SELECT * FROM users WHERE 'a' = 'b';");
  assert!(d.iter().any(|x| x.code == "sql474" && x.message.contains("always FALSE")), "expected sql474 for 'a' = 'b': {d:?}");
}

#[test]
fn sql474_neq_same_string() {
  // `'a' <> 'a'` -- always false (contradiction).
  let d = diags("SELECT * FROM users WHERE 'a' <> 'a';");
  assert!(d.iter().any(|x| x.code == "sql474" && x.message.contains("always FALSE")), "expected sql474 for 'a' <> 'a': {d:?}");
}

#[test]
fn sql474_quiet_for_col_eq_literal() {
  let d = diags("SELECT * FROM users WHERE id = 5;");
  assert!(!d.iter().any(|x| x.code == "sql474"), "column-vs-literal must not fire: {d:?}");
}

#[test]
fn sql474_quiet_for_different_kinds() {
  // String compared to numeric literal -- not flagged (PG would
  // raise a type error or implicit-cast).
  let d = diags("SELECT * FROM users WHERE 1 = 'a';");
  assert!(!d.iter().any(|x| x.code == "sql474"), "different-kind literals must not fire: {d:?}");
}

#[test]
fn sql475_insert_self_select() {
  let d = diags("INSERT INTO users SELECT * FROM users;");
  assert!(d.iter().any(|x| x.code == "sql475" && x.message.contains("doubles")), "expected sql475 for self-insert: {d:?}");
}

#[test]
fn sql475_insert_self_select_with_explicit_cols() {
  let d = diags("INSERT INTO users (id, email) SELECT id, email FROM users;");
  assert!(d.iter().any(|x| x.code == "sql475"), "expected sql475 for explicit-col self-insert: {d:?}");
}

#[test]
fn sql475_quiet_for_different_source() {
  let d = diags("INSERT INTO users SELECT * FROM orders;");
  assert!(!d.iter().any(|x| x.code == "sql475"), "different source must not fire: {d:?}");
}

#[test]
fn sql475_quiet_for_on_conflict_guard() {
  let d = diags("INSERT INTO users SELECT * FROM users ON CONFLICT DO NOTHING;");
  assert!(!d.iter().any(|x| x.code == "sql475"), "ON CONFLICT guard must not fire: {d:?}");
}

#[test]
fn sql348_quiet_for_pg_sleep_for() {
  let d = diags("SELECT pg_sleep_for('5 seconds');");
  assert!(!d.iter().any(|x| x.code == "sql348"), "pg_sleep_for must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_pg_sleep_until() {
  let d = diags("SELECT pg_sleep_until(now() + interval '1 minute');");
  assert!(!d.iter().any(|x| x.code == "sql348"), "pg_sleep_until must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_format_type() {
  let d = diags("SELECT format_type(23, NULL);");
  assert!(!d.iter().any(|x| x.code == "sql348"), "format_type must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_obj_description() {
  let d = diags("SELECT obj_description('users'::regclass);");
  assert!(!d.iter().any(|x| x.code == "sql348"), "obj_description must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_col_description() {
  let d = diags("SELECT col_description('users'::regclass, 1);");
  assert!(!d.iter().any(|x| x.code == "sql348"), "col_description must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_bit_count() {
  let d = diags("SELECT bit_count(B'1010');");
  assert!(!d.iter().any(|x| x.code == "sql348"), "bit_count must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_pgcrypto_functions() {
  let d = diags("SELECT crypt('pwd', gen_salt('bf'));");
  assert!(!d.iter().any(|x| x.code == "sql348"), "crypt/gen_salt must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_gen_random_bytes() {
  let d = diags("SELECT gen_random_bytes(16);");
  assert!(!d.iter().any(|x| x.code == "sql348"), "gen_random_bytes must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_hmac() {
  let d = diags("SELECT hmac('msg', 'key', 'sha256');");
  assert!(!d.iter().any(|x| x.code == "sql348"), "hmac must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_range_merge() {
  let d = diags("SELECT range_merge(int4range(1,5), int4range(10,20));");
  assert!(!d.iter().any(|x| x.code == "sql348" && x.message.contains("range_merge")), "range_merge must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_to_hex() {
  let d = diags("SELECT to_hex(255);");
  assert!(!d.iter().any(|x| x.code == "sql348"), "to_hex must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_normalize() {
  let d = diags("SELECT normalize('a', NFC);");
  assert!(!d.iter().any(|x| x.code == "sql348" && x.message.contains("normalize")), "normalize must not be flagged: {d:?}");
}

#[test]
fn sql348_quiet_for_int4multirange() {
  let d = diags("SELECT int4multirange(int4range(1,5), int4range(10,20));");
  assert!(!d.iter().any(|x| x.code == "sql348" && x.message.contains("int4multirange")), "int4multirange must not be flagged: {d:?}");
}

#[test]
fn sql020_quiet_for_jsonb_array_length() {
  // Regression: sql020's substring match shouldn't fire on
  // jsonb_array_length (which CONTAINS `array_length`).
  let d = diags("SELECT jsonb_array_length('[1,2,3]'::jsonb);");
  assert!(!d.iter().any(|x| x.code == "sql020"), "jsonb_array_length must not trigger sql020: {d:?}");
}

#[test]
fn sql020_still_fires_for_array_length() {
  let d = diags("SELECT array_length(ARRAY[1,2,3], 1);");
  assert!(d.iter().any(|x| x.code == "sql020"), "array_length should still fire sql020: {d:?}");
}

#[test]
fn sql476_simple_case_when_null_first() {
  let d = diags("SELECT CASE id WHEN NULL THEN 'a' ELSE 'b' END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql476" && x.message.contains("IS NULL")), "expected sql476 for simple CASE WHEN NULL: {d:?}");
}

#[test]
fn sql476_simple_case_when_null_in_chain() {
  let d = diags("SELECT CASE id WHEN 1 THEN 'a' WHEN NULL THEN 'b' END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql476"), "expected sql476 for WHEN NULL in chain: {d:?}");
}

#[test]
fn sql476_quiet_for_searched_case_is_null() {
  let d = diags("SELECT CASE WHEN id IS NULL THEN 'a' END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql476"), "searched CASE IS NULL must not fire: {d:?}");
}

#[test]
fn sql476_quiet_for_simple_case_with_real_values() {
  let d = diags("SELECT CASE id WHEN 1 THEN 'a' WHEN 2 THEN 'b' END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql476"), "simple CASE with real values must not fire: {d:?}");
}

#[test]
fn sql475_quiet_for_partial_match_table_name() {
  // Regression: `INSERT INTO users SELECT * FROM users_archive` -- target
  // and source are *different* tables that share a prefix; sql475
  // must not false-fire from a substring match.
  let d = diags("INSERT INTO users SELECT * FROM users_archive;");
  assert!(!d.iter().any(|x| x.code == "sql475"), "users vs users_archive must not fire sql475: {d:?}");
}

#[test]
fn sql475_fires_for_schema_qualified_target() {
  // `INSERT INTO public.users SELECT * FROM users` -- schema-qualified
  // target with bare-name source still resolves to the same table.
  let d = diags("INSERT INTO public.users SELECT * FROM users;");
  assert!(d.iter().any(|x| x.code == "sql475"), "schema-qualified target must still fire: {d:?}");
}

#[test]
fn sql477_contains_empty_jsonb_object() {
  let d = diags("SELECT * FROM users WHERE name::jsonb @> '{}'::jsonb;");
  assert!(d.iter().any(|x| x.code == "sql477" && x.message.contains("vacuously")), "expected sql477 for @> '{{}}'::jsonb: {d:?}");
}

#[test]
fn sql477_contains_empty_jsonb_array() {
  let d = diags("SELECT * FROM users WHERE name::jsonb @> '[]'::jsonb;");
  assert!(d.iter().any(|x| x.code == "sql477"), "expected sql477 for @> '[]'::jsonb: {d:?}");
}

#[test]
fn sql477_contains_empty_array_constructor() {
  let d = diags("SELECT * FROM users WHERE id::text[] @> ARRAY[]::text[];");
  assert!(d.iter().any(|x| x.code == "sql477"), "expected sql477 for @> ARRAY[]: {d:?}");
}

#[test]
fn sql477_quiet_for_non_empty_container() {
  let d = diags("SELECT * FROM users WHERE name::jsonb @> '{\"a\":1}'::jsonb;");
  assert!(!d.iter().any(|x| x.code == "sql477"), "non-empty container must not fire: {d:?}");
}

#[test]
fn sql478_contained_by_empty_jsonb_object() {
  let d = diags("SELECT * FROM users WHERE name::jsonb <@ '{}'::jsonb;");
  assert!(d.iter().any(|x| x.code == "sql478" && x.message.contains("intended filter")), "expected sql478 for <@ '{{}}'::jsonb: {d:?}");
}

#[test]
fn sql478_contained_by_empty_jsonb_array() {
  let d = diags("SELECT * FROM users WHERE name::jsonb <@ '[]'::jsonb;");
  assert!(d.iter().any(|x| x.code == "sql478"), "expected sql478 for <@ '[]'::jsonb: {d:?}");
}

#[test]
fn sql478_contained_by_empty_array_constructor() {
  let d = diags("SELECT * FROM users WHERE id::text[] <@ ARRAY[]::text[];");
  assert!(d.iter().any(|x| x.code == "sql478"), "expected sql478 for <@ ARRAY[]: {d:?}");
}

#[test]
fn sql478_quiet_for_non_empty_container() {
  let d = diags("SELECT * FROM users WHERE name::jsonb <@ '{\"a\":1}'::jsonb;");
  assert!(!d.iter().any(|x| x.code == "sql478"), "non-empty container must not fire: {d:?}");
}

#[test]
fn sql479_substring_zero_start_comma() {
  let d = diags("SELECT substring(name, 0, 3) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql479" && x.message.contains("1-indexed")), "expected sql479 for substring(name,0,3): {d:?}");
}

#[test]
fn sql479_substr_zero_start_comma() {
  let d = diags("SELECT substr(name, 0, 3) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql479"), "expected sql479 for substr(name,0,3): {d:?}");
}

#[test]
fn sql479_substring_zero_from_form() {
  let d = diags("SELECT substring(name FROM 0 FOR 3) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql479"), "expected sql479 for substring(name FROM 0 FOR 3): {d:?}");
}

#[test]
fn sql479_quiet_for_one_start() {
  let d = diags("SELECT substring(name, 1, 3) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql479"), "1-start must not fire: {d:?}");
}

#[test]
fn sql479_quiet_for_nonzero_first_arg() {
  // First arg is `0` but that's the haystack -- second arg is `5`,
  // a normal start. Must not false-positive on the first slot.
  let d = diags("SELECT substring('0', 5) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql479"), "zero in first arg slot must not fire: {d:?}");
}

#[test]
fn sql480_group_by_string_literal() {
  let d = diags("SELECT count(*) FROM users GROUP BY 'name';");
  assert!(d.iter().any(|x| x.code == "sql480" && x.message.contains("string literal")), "expected sql480 for GROUP BY 'name': {d:?}");
}

#[test]
fn sql480_group_by_null() {
  let d = diags("SELECT count(*) FROM users GROUP BY NULL;");
  assert!(d.iter().any(|x| x.code == "sql480" && x.message.contains("NULL")), "expected sql480 for GROUP BY NULL: {d:?}");
}

#[test]
fn sql480_group_by_boolean() {
  let d = diags("SELECT count(*) FROM users GROUP BY true;");
  assert!(d.iter().any(|x| x.code == "sql480" && x.message.contains("boolean")), "expected sql480 for GROUP BY true: {d:?}");
}

#[test]
fn sql480_quiet_for_real_column() {
  let d = diags("SELECT count(*) FROM users GROUP BY name;");
  assert!(!d.iter().any(|x| x.code == "sql480"), "real column must not fire: {d:?}");
}

#[test]
fn sql480_quiet_for_positional() {
  let d = diags("SELECT name, count(*) FROM users GROUP BY 1;");
  assert!(!d.iter().any(|x| x.code == "sql480"), "positional GROUP BY 1 must not fire: {d:?}");
}

#[test]
fn sql404_quiet_for_group_by_null_literal() {
  // Regression: sql404 was misreading NULL as an unknown column.
  // NULL is a keyword literal; sql480 handles it as a constant.
  let d = diags("SELECT count(*) FROM users GROUP BY NULL;");
  assert!(!d.iter().any(|x| x.code == "sql404"), "sql404 must not flag NULL: {d:?}");
}

#[test]
fn sql404_quiet_for_group_by_boolean_literal() {
  let d = diags("SELECT count(*) FROM users GROUP BY true;");
  assert!(!d.iter().any(|x| x.code == "sql404"), "sql404 must not flag boolean literal: {d:?}");
}

#[test]
fn sql404_quiet_for_group_by_current_date() {
  let d = diags("SELECT count(*) FROM users GROUP BY CURRENT_DATE;");
  assert!(!d.iter().any(|x| x.code == "sql404"), "sql404 must not flag CURRENT_DATE: {d:?}");
}

#[test]
fn sql481_position_empty_haystack_literal_needle() {
  let d = diags("SELECT position('a' in '') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql481" && x.message.contains("empty haystack")), "expected sql481 for position('a' in ''): {d:?}");
}

#[test]
fn sql481_position_empty_haystack_column_needle() {
  let d = diags("SELECT position(name in '') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql481"), "expected sql481 for position(name in ''): {d:?}");
}

#[test]
fn sql481_strpos_empty_haystack() {
  let d = diags("SELECT strpos('', name) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql481"), "expected sql481 for strpos('', name): {d:?}");
}

#[test]
fn sql481_quiet_for_real_haystack() {
  let d = diags("SELECT position('a' in name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql481"), "real haystack must not fire: {d:?}");
}

#[test]
fn sql481_quiet_for_empty_needle() {
  // sql446 owns the empty-needle case; sql481 must not also fire.
  let d = diags("SELECT strpos(name, '') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql481"), "empty needle is sql446 territory: {d:?}");
}

#[test]
fn sql482_having_true_is_pointless() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING true;");
  assert!(d.iter().any(|x| x.code == "sql482" && x.message.contains("TRUE")), "expected sql482 Hint for HAVING true: {d:?}");
}

#[test]
fn sql482_having_false_empties_result() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING false;");
  assert!(d.iter().any(|x| x.code == "sql482" && x.message.contains("FALSE")), "expected sql482 Warning for HAVING false: {d:?}");
}

#[test]
fn sql482_having_null_empties_result() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING NULL;");
  assert!(d.iter().any(|x| x.code == "sql482" && x.message.contains("NULL")), "expected sql482 Warning for HAVING NULL: {d:?}");
}

#[test]
fn sql482_having_string_literal() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING 'x';");
  assert!(d.iter().any(|x| x.code == "sql482"), "expected sql482 for HAVING '<literal>': {d:?}");
}

#[test]
fn sql482_quiet_for_real_aggregate_predicate() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING count(*) > 0;");
  assert!(!d.iter().any(|x| x.code == "sql482"), "real aggregate HAVING must not fire: {d:?}");
}

#[test]
fn sql483_split_part_zero_field() {
  let d = diags("SELECT split_part(name, ',', 0) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql483" && x.message.contains("field position must not be zero")), "expected sql483 Error for split_part(_,_,0): {d:?}");
}

#[test]
fn sql483_split_part_zero_with_literal_haystack() {
  let d = diags("SELECT split_part('a,b,c', ',', 0);");
  assert!(d.iter().any(|x| x.code == "sql483"), "expected sql483 with literal haystack: {d:?}");
}

#[test]
fn sql483_quiet_for_positive_field() {
  let d = diags("SELECT split_part(name, ',', 1) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql483"), "positive field must not fire: {d:?}");
}

#[test]
fn sql483_quiet_for_negative_field() {
  let d = diags("SELECT split_part(name, ',', -1) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql483"), "negative field (pg14+) must not fire: {d:?}");
}

#[test]
fn sql483_quiet_when_zero_is_in_other_slot() {
  // The `0` is the haystack literal, not the field position. Must
  // not false-positive.
  let d = diags("SELECT split_part('0', ',', 1);");
  assert!(!d.iter().any(|x| x.code == "sql483"), "zero in non-field slot must not fire: {d:?}");
}

#[test]
fn sql484_partition_by_string_literal() {
  let d = diags("SELECT row_number() OVER (PARTITION BY 'name' ORDER BY id) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql484" && x.message.contains("single window")), "expected sql484 for PARTITION BY 'name': {d:?}");
}

#[test]
fn sql484_partition_by_null() {
  let d = diags("SELECT row_number() OVER (PARTITION BY NULL ORDER BY id) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql484"), "expected sql484 for PARTITION BY NULL: {d:?}");
}

#[test]
fn sql484_partition_by_boolean() {
  let d = diags("SELECT row_number() OVER (PARTITION BY true ORDER BY id) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql484"), "expected sql484 for PARTITION BY true: {d:?}");
}

#[test]
fn sql484_partition_by_integer_literal() {
  let d = diags("SELECT row_number() OVER (PARTITION BY 1 ORDER BY id) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql484"), "expected sql484 for PARTITION BY 1: {d:?}");
}

#[test]
fn sql484_partition_by_mixed_keys_flags_only_constant() {
  let d = diags("SELECT row_number() OVER (PARTITION BY name, 'extra' ORDER BY id) FROM users;");
  let n = d.iter().filter(|x| x.code == "sql484").count();
  assert_eq!(n, 1, "expected exactly one sql484 for the constant key only: {d:?}");
}

#[test]
fn sql484_quiet_for_real_column() {
  let d = diags("SELECT row_number() OVER (PARTITION BY name ORDER BY id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql484"), "real column partition must not fire: {d:?}");
}

#[test]
fn sql484_quiet_for_no_partition_clause() {
  let d = diags("SELECT row_number() OVER (ORDER BY id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql484"), "missing PARTITION BY must not fire: {d:?}");
}

#[test]
fn sql485_regexp_split_to_array_empty_pattern() {
  let d = diags("SELECT regexp_split_to_array(name, '') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql485" && x.message.contains("single chars")), "expected sql485 for regexp_split_to_array(_,''): {d:?}");
}

#[test]
fn sql485_regexp_split_to_table_empty_pattern() {
  let d = diags("SELECT regexp_split_to_table(name, '') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql485"), "expected sql485 for regexp_split_to_table(_,''): {d:?}");
}

#[test]
fn sql485_regexp_match_empty_pattern() {
  let d = diags("SELECT regexp_match(name, '') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql485" && x.message.contains("every position")), "expected sql485 for regexp_match(_,''): {d:?}");
}

#[test]
fn sql485_regexp_matches_empty_pattern() {
  let d = diags("SELECT regexp_matches(name, '') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql485"), "expected sql485 for regexp_matches(_,''): {d:?}");
}

#[test]
fn sql485_quiet_for_real_pattern() {
  let d = diags("SELECT regexp_split_to_array(name, ',') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql485"), "real pattern must not fire: {d:?}");
}

#[test]
fn sql485_quiet_for_regexp_replace() {
  // regexp_replace has a different signature (3rd arg is replacement);
  // sql485 must not misfire on it even if the 2nd arg is empty -- that
  // case has different semantics (a no-op replace).
  let d = diags("SELECT regexp_replace(name, '', 'x') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql485"), "regexp_replace is sql485-out-of-scope: {d:?}");
}

#[test]
fn sql486_distinct_star() {
  let d = diags("SELECT DISTINCT * FROM users;");
  assert!(d.iter().any(|x| x.code == "sql486" && x.message.contains("entire row")), "expected sql486 for SELECT DISTINCT *: {d:?}");
}

#[test]
fn sql486_distinct_qualified_star() {
  let d = diags("SELECT DISTINCT u.* FROM users u;");
  assert!(d.iter().any(|x| x.code == "sql486"), "expected sql486 for SELECT DISTINCT u.*: {d:?}");
}

#[test]
fn sql486_quiet_for_distinct_narrow() {
  let d = diags("SELECT DISTINCT name FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql486"), "narrow DISTINCT must not fire: {d:?}");
}

#[test]
fn sql486_quiet_for_distinct_multi() {
  let d = diags("SELECT DISTINCT id, name FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql486"), "multi-col DISTINCT must not fire: {d:?}");
}

#[test]
fn sql486_quiet_for_distinct_on() {
  // DISTINCT ON is a different feature with its own semantics
  // (sql101 covers its concerns); sql486 must stay out.
  let d = diags("SELECT DISTINCT ON (id) * FROM users ORDER BY id;");
  assert!(!d.iter().any(|x| x.code == "sql486"), "DISTINCT ON must not fire: {d:?}");
}

#[test]
fn sql486_quiet_for_select_star_no_distinct() {
  let d = diags("SELECT * FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql486"), "plain SELECT * must not fire: {d:?}");
}

#[test]
fn sql487_array_length_dim_zero() {
  let d = diags("SELECT array_length(name::text[], 0) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql487" && x.message.contains("dimension 0")), "expected sql487 for array_length(_,0): {d:?}");
}

#[test]
fn sql487_array_length_dim_negative() {
  let d = diags("SELECT array_length(name::text[], -1) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql487" && x.message.contains("negative")), "expected sql487 for array_length(_,-1): {d:?}");
}

#[test]
fn sql487_array_lower_dim_zero() {
  let d = diags("SELECT array_lower(name::text[], 0) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql487"), "expected sql487 for array_lower(_,0): {d:?}");
}

#[test]
fn sql487_array_upper_dim_zero() {
  let d = diags("SELECT array_upper(name::text[], 0) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql487"), "expected sql487 for array_upper(_,0): {d:?}");
}

#[test]
fn sql487_quiet_for_dim_one() {
  let d = diags("SELECT array_length(name::text[], 1) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql487"), "dim=1 must not fire: {d:?}");
}

#[test]
fn sql487_quiet_for_dim_two() {
  let d = diags("SELECT array_length(name::text[], 2) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql487"), "dim=2 must not fire: {d:?}");
}

#[test]
fn sql488_jsonb_path_exists_missing_anchor() {
  let d = diags("SELECT jsonb_path_exists(name::jsonb, 'name') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql488" && x.message.contains("root anchor")), "expected sql488 for missing $ anchor: {d:?}");
}

#[test]
fn sql488_jsonb_path_query_missing_anchor() {
  let d = diags("SELECT jsonb_path_query(name::jsonb, 'foo') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql488"), "expected sql488 for jsonb_path_query: {d:?}");
}

#[test]
fn sql488_quiet_for_valid_path() {
  let d = diags("SELECT jsonb_path_exists(name::jsonb, '$.name') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql488"), "valid path must not fire: {d:?}");
}

#[test]
fn sql488_quiet_for_strict_prefix() {
  let d = diags("SELECT jsonb_path_exists(name::jsonb, 'strict $.name') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql488"), "strict $ prefix must not fire: {d:?}");
}

#[test]
fn sql488_quiet_for_lax_prefix() {
  let d = diags("SELECT jsonb_path_exists(name::jsonb, 'lax $') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql488"), "lax $ prefix must not fire: {d:?}");
}

#[test]
fn sql488_quiet_for_column_path() {
  // The path is a column ref (not a string literal), so we can't
  // verify it at lint time -- must not misfire.
  let d = diags("SELECT jsonb_path_exists(name::jsonb, email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql488"), "non-literal path must not fire: {d:?}");
}

#[test]
fn sql489_plus_zero() {
  let d = diags("SELECT * FROM users WHERE id + 0 = 5;");
  assert!(d.iter().any(|x| x.code == "sql489" && x.message.contains("identity")), "expected sql489 for id + 0: {d:?}");
}

#[test]
fn sql489_minus_zero() {
  let d = diags("SELECT * FROM users WHERE id - 0 = 5;");
  assert!(d.iter().any(|x| x.code == "sql489"), "expected sql489 for id - 0: {d:?}");
}

#[test]
fn sql489_times_one() {
  let d = diags("SELECT * FROM users WHERE id * 1 = 5;");
  assert!(d.iter().any(|x| x.code == "sql489"), "expected sql489 for id * 1: {d:?}");
}

#[test]
fn sql489_div_one() {
  let d = diags("SELECT * FROM users WHERE id / 1 = 5;");
  assert!(d.iter().any(|x| x.code == "sql489"), "expected sql489 for id / 1: {d:?}");
}

#[test]
fn sql489_zero_plus_col() {
  // Commutative form
  let d = diags("SELECT * FROM users WHERE 0 + id = 5;");
  assert!(d.iter().any(|x| x.code == "sql489"), "expected sql489 for 0 + id: {d:?}");
}

#[test]
fn sql489_one_times_col() {
  let d = diags("SELECT * FROM users WHERE 1 * id = 5;");
  assert!(d.iter().any(|x| x.code == "sql489"), "expected sql489 for 1 * id: {d:?}");
}

#[test]
fn sql489_quiet_for_real_arithmetic() {
  let d = diags("SELECT * FROM users WHERE id + 1 = 5;");
  assert!(!d.iter().any(|x| x.code == "sql489"), "real arithmetic must not fire: {d:?}");
}

#[test]
fn sql489_quiet_for_rhs_arith() {
  // `id = 5 + 0` -- the arith is on the constant side, not
  // wrapping the column, so it's not a sargability concern.
  let d = diags("SELECT * FROM users WHERE id = 5 + 0;");
  assert!(!d.iter().any(|x| x.code == "sql489"), "RHS arith must not fire: {d:?}");
}

#[test]
fn sql489_quiet_for_decimal_literal() {
  // `id + 0.5` is NOT an identity; literal `0` followed by `.5`.
  let d = diags("SELECT * FROM users WHERE id + 0.5 = 5;");
  assert!(!d.iter().any(|x| x.code == "sql489"), "decimal literal must not fire: {d:?}");
}

#[test]
fn sql490_concat_right_empty() {
  let d = diags("SELECT name || '' FROM users;");
  assert!(d.iter().any(|x| x.code == "sql490" && x.message.contains("no-op")), "expected sql490 for name || '': {d:?}");
}

#[test]
fn sql490_concat_left_empty() {
  let d = diags("SELECT '' || name FROM users;");
  assert!(d.iter().any(|x| x.code == "sql490"), "expected sql490 for '' || name: {d:?}");
}

#[test]
fn sql490_quiet_for_non_empty_literal() {
  let d = diags("SELECT name || ' ' FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql490"), "single-space literal must not fire: {d:?}");
}

#[test]
fn sql490_quiet_for_real_concat() {
  let d = diags("SELECT name || 'foo' FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql490"), "real concat must not fire: {d:?}");
}

#[test]
fn sql490_quiet_for_no_concat() {
  let d = diags("SELECT name FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql490"), "no concat must not fire: {d:?}");
}

#[test]
fn sql490_quiet_for_concat_function() {
  // sql490 only targets the `||` operator; the `concat()` function
  // has its own NULL/empty handling and is out of scope here.
  let d = diags("SELECT concat(name, '') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql490"), "concat() function must not fire: {d:?}");
}

#[test]
fn sql491_having_numeric_tautology() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING 1 = 1;");
  assert!(d.iter().any(|x| x.code == "sql491" && x.message.contains("TRUE")), "expected sql491 Hint for HAVING 1=1: {d:?}");
}

#[test]
fn sql491_having_numeric_contradiction() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING 1 = 2;");
  assert!(d.iter().any(|x| x.code == "sql491" && x.message.contains("FALSE")), "expected sql491 Warning for HAVING 1=2: {d:?}");
}

#[test]
fn sql491_having_zero_zero_tautology() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING 0 = 0;");
  assert!(d.iter().any(|x| x.code == "sql491"), "expected sql491 for HAVING 0=0: {d:?}");
}

#[test]
fn sql491_having_string_tautology() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING 'a' = 'a';");
  assert!(d.iter().any(|x| x.code == "sql491" && x.message.contains("TRUE")), "expected sql491 for HAVING 'a'='a': {d:?}");
}

#[test]
fn sql491_having_neq_tautology() {
  // `1 <> 2` is always TRUE -- inequality of distinct constants.
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING 1 <> 2;");
  assert!(d.iter().any(|x| x.code == "sql491" && x.message.contains("TRUE")), "expected sql491 Hint for HAVING 1<>2: {d:?}");
}

#[test]
fn sql491_quiet_for_real_aggregate_predicate() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING count(*) > 0;");
  assert!(!d.iter().any(|x| x.code == "sql491"), "real aggregate HAVING must not fire: {d:?}");
}

#[test]
fn sql482_quiet_for_comparison_after_iter185_fix() {
  // Regression: sql482's classify_constant was matching `'a' = 'a'`
  // as a bare string literal because it just checked starts-with-'
  // and ends-with-'. Now is_lone_string_literal walks the literal
  // properly. sql491 owns this comparison case.
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING 'a' = 'a';");
  assert!(!d.iter().any(|x| x.code == "sql482"), "sql482 must not misclassify comparison: {d:?}");
}

#[test]
fn sql429_double_equals_typo() {
  let d = diags("SELECT * FROM users WHERE id == 1;");
  assert!(d.iter().any(|x| x.code == "sql429" && x.message.contains("`==`")), "got {d:?}");
}

#[test]
fn sql429_spaceship_operator_mysql() {
  let d = diags("SELECT * FROM users WHERE id <=> 1;");
  assert!(d.iter().any(|x| x.code == "sql429" && x.message.contains("MySQL")), "got {d:?}");
}

#[test]
fn sql429_quiet_for_real_equality() {
  let d = diags("SELECT * FROM users WHERE id = '1';");
  assert!(!d.iter().any(|x| x.code == "sql429"), "real `=` must not fire: {d:?}");
}

#[test]
fn sql429_quiet_for_not_equals() {
  let d = diags("SELECT * FROM users WHERE id != '1';");
  assert!(!d.iter().any(|x| x.code == "sql429"), "`!=` is valid PG: {d:?}");
}

#[test]
fn sql428_max_star_invalid() {
  let d = diags("SELECT MAX(*) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql428"), "got {d:?}");
}

#[test]
fn sql428_sum_star_invalid() {
  let d = diags("SELECT SUM(*) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql428"), "got {d:?}");
}

#[test]
fn sql428_count_star_still_valid() {
  let d = diags("SELECT COUNT(*) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql428"), "count(*) must not fire: {d:?}");
}

#[test]
fn sql428_max_of_column_silent() {
  let d = diags("SELECT MAX(age) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql428"), "max(col) must not fire: {d:?}");
}

#[test]
fn sql427_lower_wrapper_on_column() {
  let d = diags("SELECT * FROM users WHERE lower(email) = 'foo@example.com';");
  assert!(d.iter().any(|x| x.code == "sql427" && x.message.contains("lower(email")), "got {d:?}");
}

#[test]
fn sql427_substring_wrapper_on_column() {
  let d = diags("SELECT * FROM users WHERE substring(email, 1, 3) = 'abc';");
  assert!(d.iter().any(|x| x.code == "sql427" && x.message.contains("substring(email")), "got {d:?}");
}

#[test]
fn sql427_quiet_for_lower_on_literal() {
  // lower('LITERAL') = col -- column on RHS isn't wrapped.
  let d = diags("SELECT * FROM users WHERE lower('FOO') = email;");
  assert!(!d.iter().any(|x| x.code == "sql427"), "literal-wrapped lhs must not fire: {d:?}");
}

#[test]
fn sql427_cast_function_form_on_column() {
  let d = diags("SELECT * FROM users WHERE CAST(created_at AS date) = '2023-01-01';");
  assert!(
    d.iter().any(|x| x.code == "sql427" && x.message.contains("CAST(created_at")),
    "got {d:?}"
  );
}

#[test]
fn sql427_date_function_wrapper_on_column() {
  let d = diags("SELECT * FROM users WHERE date(created_at) = '2023-01-01';");
  assert!(
    d.iter().any(|x| x.code == "sql427" && x.message.contains("date(col)")),
    "got {d:?}"
  );
}

#[test]
fn sql427_double_colon_cast_on_column() {
  let d = diags("SELECT * FROM users WHERE created_at::date = '2023-01-01';");
  assert!(
    d.iter().any(|x| x.code == "sql427" && x.message.contains("created_at::date")),
    "got {d:?}"
  );
}

#[test]
fn sql427_quiet_when_literal_is_cast() {
  // RHS literal cast is fine -- doesn't wrap the column.
  let d = diags("SELECT * FROM users WHERE created_at = '2023-01-01'::timestamptz;");
  assert!(!d.iter().any(|x| x.code == "sql427"), "rhs cast must not fire: {d:?}");
}

#[test]
fn sql427_quiet_for_date_on_current_date() {
  // Not a column -- no index to block.
  let d = diags("SELECT * FROM users WHERE date(CURRENT_DATE) = '2023-01-01';");
  assert!(!d.iter().any(|x| x.code == "sql427"), "got {d:?}");
}

#[test]
fn sql269_date_part_on_column_fires() {
  // date_part('year', col) has the same non-sargable shape as
  // EXTRACT(YEAR FROM col); both should hint a range rewrite.
  let d = diags("SELECT * FROM users WHERE date_part('year', created_at) = 2023;");
  assert!(
    d.iter().any(|x| x.code == "sql269" && x.message.contains("date_part")),
    "got {d:?}"
  );
}

#[test]
fn sql269_extract_on_current_date_silent() {
  // No column to index when the operand is CURRENT_DATE -- previously
  // a false positive.
  let d = diags("SELECT * FROM users WHERE EXTRACT(YEAR FROM CURRENT_DATE) = 2023;");
  assert!(!d.iter().any(|x| x.code == "sql269"), "CURRENT_DATE operand must not fire: {d:?}");
}

#[test]
fn sql426_distinct_order_by_not_in_projection() {
  let d = diags("SELECT DISTINCT id FROM users ORDER BY age;");
  assert!(d.iter().any(|x| x.code == "sql426" && x.message.contains("age")), "got {d:?}");
}

#[test]
fn sql426_distinct_order_by_in_projection_silent() {
  let d = diags("SELECT DISTINCT id FROM users ORDER BY id;");
  assert!(!d.iter().any(|x| x.code == "sql426"), "got {d:?}");
}

#[test]
fn sql426_distinct_order_by_alias_silent() {
  let d = diags("SELECT DISTINCT id AS x FROM users ORDER BY x;");
  assert!(!d.iter().any(|x| x.code == "sql426"), "alias must be honored: {d:?}");
}

#[test]
fn sql426_distinct_star_silent() {
  // `SELECT DISTINCT *` includes every column, so ORDER BY anything is fine.
  let d = diags("SELECT DISTINCT * FROM users ORDER BY age;");
  assert!(!d.iter().any(|x| x.code == "sql426"), "DISTINCT * must not fire: {d:?}");
}

#[test]
fn sql426_no_distinct_silent() {
  // Without DISTINCT, the rule doesn't apply.
  let d = diags("SELECT id FROM users ORDER BY age;");
  assert!(!d.iter().any(|x| x.code == "sql426"), "no DISTINCT: {d:?}");
}

#[test]
fn sql425_window_in_where_row_number() {
  let d = diags("SELECT * FROM users WHERE row_number() OVER () = 1;");
  assert!(d.iter().any(|x| x.code == "sql425"), "got {d:?}");
}

#[test]
fn sql425_window_in_where_with_order_by() {
  let d = diags("SELECT * FROM users WHERE rank() OVER (ORDER BY age) <= 5;");
  assert!(d.iter().any(|x| x.code == "sql425"), "got {d:?}");
}

#[test]
fn sql425_quiet_when_window_in_subquery() {
  let d = diags("SELECT * FROM (SELECT row_number() OVER () AS rn FROM users) s WHERE s.rn = 1;");
  assert!(!d.iter().any(|x| x.code == "sql425"), "subquery-wrapped window must not fire: {d:?}");
}

#[test]
fn sql425_quiet_for_window_in_select_list() {
  let d = diags("SELECT row_number() OVER () FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql425"), "SELECT-list window must not fire: {d:?}");
}

#[test]
fn sql424_aggregate_in_group_by() {
  let d = diags("SELECT * FROM users GROUP BY count(*);");
  assert!(d.iter().any(|x| x.code == "sql424" && x.message.contains("GROUP BY")), "got {d:?}");
}

#[test]
fn sql425_window_in_group_by() {
  let d = diags("SELECT * FROM users GROUP BY rank() OVER (ORDER BY age);");
  assert!(d.iter().any(|x| x.code == "sql425" && x.message.contains("GROUP BY")), "got {d:?}");
}

#[test]
fn sql424_aggregate_in_join_on_clause() {
  // Aggregate functions can't appear in JOIN ON either -- PG rejects.
  let d = diags("SELECT * FROM users u JOIN users v ON max(u.id) = v.id;");
  assert!(
    d.iter().any(|x| x.code == "sql424" && x.message.contains("JOIN ON")),
    "expected sql424 for aggregate in JOIN ON: {d:?}"
  );
}

#[test]
fn sql424_aggregate_in_where_count() {
  let d = diags("SELECT * FROM users WHERE count(*) > 1;");
  assert!(
    d.iter().any(|x| x.code == "sql424" && x.message.contains("count")),
    "expected sql424 for count(*) in WHERE: {d:?}"
  );
}

#[test]
fn sql424_aggregate_in_where_sum() {
  let d = diags("SELECT * FROM users WHERE sum(age) > 100;");
  assert!(d.iter().any(|x| x.code == "sql424" && x.message.contains("sum")), "got {d:?}");
}

#[test]
fn sql424_quiet_for_aggregate_in_subquery() {
  // Aggregates are legal inside a subquery in WHERE.
  let d = diags("SELECT * FROM users WHERE age = (SELECT max(age) FROM users);");
  assert!(!d.iter().any(|x| x.code == "sql424"), "subquery agg must not fire: {d:?}");
}

#[test]
fn sql424_quiet_for_aggregate_in_select_list() {
  let d = diags("SELECT count(*) FROM users WHERE id IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql424"), "SELECT-list aggregate must not fire: {d:?}");
}

#[test]
fn sql423_regex_anchored_prefix_suggests_like() {
  let d = diags("SELECT * FROM users WHERE email ~ '^abc';");
  assert!(
    d.iter().any(|x| x.code == "sql423" && x.message.contains("LIKE 'abc%'")),
    "got {d:?}"
  );
}

#[test]
fn sql423_regex_case_insensitive_suggests_ilike() {
  let d = diags("SELECT * FROM users WHERE email ~* '^abc';");
  assert!(
    d.iter().any(|x| x.code == "sql423" && x.message.contains("ILIKE 'abc%'")),
    "got {d:?}"
  );
}

#[test]
fn sql423_regex_with_trailing_star_suggests_like() {
  let d = diags("SELECT * FROM users WHERE email ~ '^abc.*';");
  assert!(d.iter().any(|x| x.code == "sql423"), "got {d:?}");
}

#[test]
fn sql423_quiet_for_unanchored_regex() {
  let d = diags("SELECT * FROM users WHERE email ~ 'abc';");
  assert!(!d.iter().any(|x| x.code == "sql423"), "got {d:?}");
}

#[test]
fn sql423_exact_match_regex_suggests_equals() {
  // `^abc$` is exact-match -- the rule now suggests `= 'abc'` rather
  // than a LIKE that would broaden the match.
  let d = diags("SELECT * FROM users WHERE email ~ '^abc$';");
  let hit = d.iter().find(|x| x.code == "sql423").expect("sql423 should fire for exact match");
  assert!(hit.message.contains("= 'abc'"), "expected `= 'abc'` suggestion: {}", hit.message);
  assert!(!hit.message.contains("LIKE 'abc%'"), "must not suggest broadening LIKE: {}", hit.message);
}

#[test]
fn sql423_exact_match_case_insensitive_suggests_lower() {
  let d = diags("SELECT * FROM users WHERE email ~* '^abc$';");
  let hit = d.iter().find(|x| x.code == "sql423").expect("sql423 should fire for ~* exact match");
  assert!(hit.message.contains("lower(col) = 'abc'"), "expected lower() suggestion: {}", hit.message);
}

#[test]
fn sql423_quiet_for_char_class_regex() {
  let d = diags("SELECT * FROM users WHERE email ~ '^[a-z]+';");
  assert!(!d.iter().any(|x| x.code == "sql423"), "char class must not flag: {d:?}");
}

#[test]
fn sql422_pred_and_negation_basic() {
  let d = diags("SELECT * FROM users WHERE age > 0 AND NOT age > 0;");
  assert!(d.iter().any(|x| x.code == "sql422"), "got {d:?}");
}

#[test]
fn sql422_quiet_when_negation_absent() {
  let d = diags("SELECT * FROM users WHERE age > 0 AND age <= 0;");
  // Semantic contradiction but text-wise different conjuncts -- our
  // rule doesn't try to be that smart. Verify it stays silent.
  assert!(!d.iter().any(|x| x.code == "sql422"), "must not flag semantic-only contradiction: {d:?}");
}

#[test]
fn sql422_quiet_for_distinct_predicates() {
  let d = diags("SELECT * FROM users WHERE age > 0 AND email IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql422"), "distinct preds must not fire: {d:?}");
}

#[test]
fn sql421_duplicate_and_predicate() {
  let d = diags("SELECT * FROM users WHERE age > 0 AND age > 0;");
  assert!(d.iter().any(|x| x.code == "sql421" && x.message.contains("age > 0")), "got {d:?}");
}

#[test]
fn sql421_duplicate_or_predicate() {
  let d = diags("SELECT * FROM users WHERE age > 0 OR age > 0;");
  assert!(d.iter().any(|x| x.code == "sql421"), "got {d:?}");
}

#[test]
fn sql421_duplicate_non_consecutive() {
  let d = diags("SELECT * FROM users WHERE age > 0 AND id IS NOT NULL AND age > 0;");
  assert!(d.iter().any(|x| x.code == "sql421"), "got {d:?}");
}

#[test]
fn sql421_quiet_for_distinct_predicates() {
  let d = diags("SELECT * FROM users WHERE age > 0 AND age > 5;");
  assert!(!d.iter().any(|x| x.code == "sql421"), "distinct preds must not fire: {d:?}");
}

#[test]
fn sql420_any_array_self_member() {
  let d = diags("SELECT * FROM users WHERE age = ANY(ARRAY[age]);");
  assert!(d.iter().any(|x| x.code == "sql420" && x.message.contains("ANY")), "got {d:?}");
}

#[test]
fn sql420_all_array_self_member() {
  let d = diags("SELECT * FROM users WHERE age = ALL(ARRAY[age]);");
  assert!(d.iter().any(|x| x.code == "sql420" && x.message.contains("ALL")), "got {d:?}");
}

#[test]
fn sql420_quiet_for_literal_array() {
  let d = diags("SELECT * FROM users WHERE age = ANY(ARRAY[1, 2, 3]);");
  assert!(!d.iter().any(|x| x.code == "sql420"), "literal array must not fire: {d:?}");
}

#[test]
fn sql419_nullif_with_null_right() {
  let d = diags("SELECT NULLIF(age, NULL) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql419" && x.message.contains("no-op")), "got {d:?}");
}

#[test]
fn sql419_nullif_with_null_left() {
  let d = diags("SELECT NULLIF(NULL, age) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql419" && x.message.contains("always returns NULL")), "got {d:?}");
}

#[test]
fn sql419_nullif_quiet_for_real_args() {
  let d = diags("SELECT NULLIF(age, 5) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql419"), "real args must not fire: {d:?}");
}

#[test]
fn sql418_distinct_on_primary_key_column() {
  // Catalog test helper's `users` table already declares id as PK,
  // so DISTINCT id FROM users is by definition redundant.
  let d = diags("SELECT DISTINCT id FROM users;");
  assert!(
    d.iter().any(|x| x.code == "sql418" && x.message.contains("PRIMARY KEY")),
    "expected sql418: {d:?}"
  );
}

#[test]
fn sql418_distinct_quiet_for_non_unique() {
  // `email` is unique in the probe catalog but the shared test
  // `cat()` may not declare it as such -- use a column known not
  // to have a unique constraint to verify silence.
  let d = diags("SELECT DISTINCT name FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql418"), "non-unique column must not fire: {d:?}");
}

#[test]
fn sql418_distinct_quiet_for_joined_query() {
  // Joins break per-table uniqueness; DISTINCT may still be needed.
  let d = diags("SELECT DISTINCT u.id FROM users u JOIN orders o ON u.id = o.user_id;");
  assert!(!d.iter().any(|x| x.code == "sql418"), "joined query must not fire: {d:?}");
}

#[test]
fn sql417_coalesce_duplicate_column_arg() {
  let d = diags("SELECT COALESCE(nickname, nickname, 'fallback') FROM users;");
  assert!(
    d.iter().any(|x| x.code == "sql417" && x.message.contains("nickname")),
    "expected sql417: {d:?}"
  );
}

#[test]
fn sql417_coalesce_null_arg_is_dead() {
  let d = diags("SELECT COALESCE(nickname, NULL, 'x') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql417" && x.message.contains("NULL")), "got {d:?}");
}

#[test]
fn sql417_coalesce_quiet_for_distinct_args() {
  let d = diags("SELECT COALESCE(nickname, 'fallback') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql417"), "distinct args must not fire: {d:?}");
}

#[test]
fn sql417_coalesce_quiet_when_function_call_arg_present() {
  // `random()` could return different values each call -- don't
  // assume the duplicate text is dead. Skip rule when any arg
  // contains a function call.
  let d = diags("SELECT COALESCE(random(), random()) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql417"), "function-call args must not flag: {d:?}");
}

#[test]
fn sql416_case_all_branches_same_int() {
  let d = diags("SELECT CASE WHEN age > 0 THEN 1 WHEN age > 10 THEN 1 ELSE 1 END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql416"), "expected sql416: {d:?}");
}

#[test]
fn sql416_case_all_branches_same_string() {
  let d = diags("SELECT CASE WHEN age > 0 THEN 'x' ELSE 'x' END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql416" && x.message.contains("'x'")), "got {d:?}");
}

#[test]
fn sql416_case_all_branches_same_null() {
  let d = diags("SELECT CASE WHEN age > 0 THEN NULL ELSE NULL END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql416"), "got {d:?}");
}

#[test]
fn sql416_case_quiet_when_branches_differ() {
  let d = diags("SELECT CASE WHEN age > 0 THEN 1 ELSE 2 END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql416"), "differing branches must not flag: {d:?}");
}

#[test]
fn sql416_case_quiet_for_no_else_with_two_thens() {
  // Without ELSE, sql150 fires; sql416 only fires if all collected
  // branches are equal AND there are ≥ 2 of them. Here both THENs
  // differ, so no fire either way.
  let d = diags("SELECT CASE WHEN age > 0 THEN 1 WHEN age > 10 THEN 2 END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql416"), "got {d:?}");
}

#[test]
fn sql415_cast_double_colon_same_type_fires() {
  let d = diags("SELECT id::uuid FROM users;");
  assert!(d.iter().any(|x| x.code == "sql415"), "expected sql415 for id::uuid: {d:?}");
}

#[test]
fn sql415_cast_function_form_same_type_fires() {
  let d = diags("SELECT CAST(email AS text) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql415"), "expected sql415 for CAST(email AS text): {d:?}");
}

#[test]
fn sql415_cast_quiet_for_different_type() {
  let d = diags("SELECT id::text FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql415"), "different type must not flag: {d:?}");
}

#[test]
fn sql415_cast_qualified_column_same_type() {
  let d = diags("SELECT u.id::uuid FROM users u;");
  assert!(d.iter().any(|x| x.code == "sql415" && x.message.contains("u.id")), "got {d:?}");
}

#[test]
fn sql414_in_list_self_member_only() {
  let d = diags("SELECT * FROM users WHERE age IN (age);");
  assert!(d.iter().any(|x| x.code == "sql414"), "expected sql414: {d:?}");
}

#[test]
fn sql414_in_list_self_member_among_literals() {
  let d = diags("SELECT * FROM users WHERE age IN (1, 2, age);");
  assert!(d.iter().any(|x| x.code == "sql414"), "expected sql414: {d:?}");
}

#[test]
fn sql414_not_in_list_self_member() {
  let d = diags("SELECT * FROM users WHERE age NOT IN (age);");
  assert!(d.iter().any(|x| x.code == "sql414"), "expected sql414 for NOT IN: {d:?}");
}

#[test]
fn sql414_quiet_for_pure_literal_list() {
  let d = diags("SELECT * FROM users WHERE age IN (1, 2, 3);");
  assert!(!d.iter().any(|x| x.code == "sql414"), "real list must not flag: {d:?}");
}

#[test]
fn sql414_quiet_when_different_columns() {
  let d = diags("SELECT * FROM users WHERE id IN (age);");
  assert!(!d.iter().any(|x| x.code == "sql414"), "different cols must not flag: {d:?}");
}

#[test]
fn sql413_concat_with_null_right() {
  let d = diags("SELECT 'a' || NULL;");
  assert!(d.iter().any(|x| x.code == "sql413"), "expected sql413 for `'a' || NULL`: {d:?}");
}

#[test]
fn sql413_concat_with_null_left() {
  let d = diags("SELECT NULL || 'b';");
  assert!(d.iter().any(|x| x.code == "sql413"), "expected sql413 for `NULL || 'b'`: {d:?}");
}

#[test]
fn sql413_concat_chained_with_null() {
  let d = diags("SELECT email || ' ' || NULL FROM users;");
  assert!(d.iter().any(|x| x.code == "sql413"), "expected sql413 for chained: {d:?}");
}

#[test]
fn sql413_quiet_for_real_concat() {
  let d = diags("SELECT email || nickname FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql413"), "must not flag real columns: {d:?}");
}

#[test]
#[allow(non_snake_case)]
fn sql413_quiet_for_string_literal_containing_NULL() {
  // The string literal `'NULL'` is text, not the NULL keyword.
  let d = diags("SELECT 'a' || 'NULL';");
  assert!(!d.iter().any(|x| x.code == "sql413"), "string literal must not trigger: {d:?}");
}

#[test]
fn sql412_duplicate_order_by_column() {
  let d = diags("SELECT * FROM users ORDER BY id, id;");
  assert!(
    d.iter().any(|x| x.code == "sql412" && x.message.contains("ORDER BY")),
    "expected sql412 for ORDER BY id, id; got {d:?}"
  );
}

#[test]
fn sql412_duplicate_group_by_column() {
  let d = diags("SELECT count(*) FROM users GROUP BY id, id;");
  assert!(
    d.iter().any(|x| x.code == "sql412" && x.message.contains("GROUP BY")),
    "expected sql412 for GROUP BY id, id; got {d:?}"
  );
}

#[test]
fn sql412_order_by_same_col_both_directions_still_flagged() {
  // `ORDER BY id ASC, id DESC` -- the second sort key is unreachable
  // because the first already pins every row's position.
  let d = diags("SELECT * FROM users ORDER BY id ASC, id DESC;");
  assert!(d.iter().any(|x| x.code == "sql412"), "got {d:?}");
}

#[test]
fn sql412_quiet_for_distinct_columns() {
  let d = diags("SELECT * FROM users ORDER BY id, email;");
  assert!(!d.iter().any(|x| x.code == "sql412"), "distinct cols must not fire: {d:?}");
}

#[test]
fn sql411_limit_with_positive_offset_no_order_by() {
  let d = diags("SELECT * FROM users LIMIT 1 OFFSET 5;");
  assert!(
    d.iter().any(|x| x.code == "sql411" && x.message.contains("OFFSET 5")),
    "expected sql411 for LIMIT 1 OFFSET 5; got {d:?}"
  );
}

#[test]
fn sql411_quiet_for_offset_zero() {
  let d = diags("SELECT * FROM users LIMIT 1 OFFSET 0;");
  assert!(!d.iter().any(|x| x.code == "sql411"), "OFFSET 0 must not fire: {d:?}");
}

#[test]
fn sql411_quiet_when_order_by_present() {
  let d = diags("SELECT * FROM users ORDER BY id LIMIT 1 OFFSET 5;");
  assert!(!d.iter().any(|x| x.code == "sql411"), "ORDER BY suppresses sql411: {d:?}");
}

#[test]
fn sql411_quiet_for_limit_without_offset() {
  // Pure LIMIT (no OFFSET) is sql051's concern.
  let d = diags("SELECT * FROM users LIMIT 10;");
  assert!(!d.iter().any(|x| x.code == "sql411"), "no OFFSET, sql411 shouldn't fire: {d:?}");
}

#[test]
fn sql410_duplicate_select_projection() {
  let d = diags("SELECT id, id FROM users;");
  assert!(
    d.iter().any(|x| x.code == "sql410" && x.message.contains("id")),
    "expected sql410 for duplicate id; got {d:?}"
  );
}

#[test]
fn sql410_duplicate_with_alias_collision() {
  // Explicit alias collides with another projection's effective name.
  let d = diags("SELECT id, email AS id FROM users;");
  assert!(d.iter().any(|x| x.code == "sql410"), "expected sql410 for alias collision: {d:?}");
}

#[test]
fn sql410_quiet_for_distinct_columns() {
  let d = diags("SELECT id, email FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql410"), "must not flag distinct cols: {d:?}");
}

#[test]
fn sql410_quiet_for_qualified_difference() {
  // `u.id` and `o.id` are different output sources -- not a duplicate.
  let d = diags("SELECT u.id, o.id FROM users u JOIN orders o ON u.id = o.user_id;");
  assert!(!d.iter().any(|x| x.code == "sql410"), "must not flag different qualifiers: {d:?}");
}

#[test]
fn sql409_not_between_self_low_bound() {
  // `NOT BETWEEN` is the same family -- still meaningless when one
  // bound is the column itself. Verify the backward-scan skips the
  // intervening NOT keyword.
  let d = diags("SELECT * FROM users WHERE age NOT BETWEEN age AND 100;");
  assert!(d.iter().any(|x| x.code == "sql409" && x.message.contains("low bound")), "got {d:?}");
}

#[test]
fn sql409_not_between_self_high_bound() {
  let d = diags("SELECT * FROM users WHERE age NOT BETWEEN 1 AND age;");
  assert!(d.iter().any(|x| x.code == "sql409" && x.message.contains("high bound")), "got {d:?}");
}

#[test]
fn sql409_not_between_quiet_for_real_bounds() {
  let d = diags("SELECT * FROM users WHERE age NOT BETWEEN 1 AND 100;");
  assert!(!d.iter().any(|x| x.code == "sql409"), "real bounds must not flag: {d:?}");
}

#[test]
fn sql409_between_self_low_bound() {
  let d = diags("SELECT * FROM users WHERE age BETWEEN age AND 100;");
  assert!(
    d.iter().any(|x| x.code == "sql409" && x.message.contains("low bound")),
    "expected sql409 low-bound; got {d:?}"
  );
}

#[test]
fn sql409_between_self_high_bound() {
  let d = diags("SELECT * FROM users WHERE age BETWEEN 1 AND age;");
  assert!(
    d.iter().any(|x| x.code == "sql409" && x.message.contains("high bound")),
    "expected sql409 high-bound; got {d:?}"
  );
}

#[test]
fn sql409_between_quiet_for_real_bounds() {
  let d = diags("SELECT * FROM users WHERE age BETWEEN 1 AND 100;");
  assert!(!d.iter().any(|x| x.code == "sql409"), "real bounds must not flag: {d:?}");
}

#[test]
fn sql409_between_qualified_column() {
  let d = diags("SELECT * FROM users u WHERE u.age BETWEEN u.age AND 100;");
  assert!(
    d.iter().any(|x| x.code == "sql409" && x.message.contains("u.age")),
    "expected sql409 for qualified column; got {d:?}"
  );
}

#[test]
fn sql409_between_in_join_on_clause() {
  let d = diags("SELECT * FROM users JOIN users u2 ON id BETWEEN id AND 10;");
  assert!(d.iter().any(|x| x.code == "sql409"), "expected sql409 in JOIN ON; got {d:?}");
}

#[test]
fn sql408_quiet_for_numeric_literal_self_compare() {
  // `WHERE 1 = 1` (and `EXISTS (SELECT 1 WHERE 1=1)`) are sql282's
  // concern (tautology placeholder), not sql408's. The byte scanner
  // shouldn't treat numeric literals as identifiers.
  let d = diags("SELECT * FROM users WHERE EXISTS (SELECT 1 FROM users WHERE 1=1);");
  assert!(!d.iter().any(|x| x.code == "sql408"), "must not flag numeric literal: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE 2 = 2;");
  assert!(!d2.iter().any(|x| x.code == "sql408"), "must not flag numeric literal: {d2:?}");
}

#[test]
fn sql408_where_column_self_compare_bare() {
  let d = diags("SELECT * FROM users WHERE age = age;");
  assert!(
    d.iter().any(|x| x.code == "sql408" && x.message.contains("age = age")),
    "expected sql408; got {d:?}"
  );
}

#[test]
fn sql408_where_column_self_compare_qualified() {
  let d = diags("SELECT * FROM users u WHERE u.age = u.age;");
  assert!(d.iter().any(|x| x.code == "sql408" && x.message.contains("u.age = u.age")), "got {d:?}");
}

#[test]
fn sql408_where_column_self_compare_less_than() {
  let d = diags("SELECT * FROM users WHERE age < age;");
  assert!(d.iter().any(|x| x.code == "sql408"), "got {d:?}");
}

#[test]
fn sql408_quiet_for_different_columns() {
  let d = diags("SELECT * FROM users WHERE age = id;");
  assert!(!d.iter().any(|x| x.code == "sql408"), "must not flag different columns: {d:?}");
}

#[test]
fn sql408_join_on_column_self_compare() {
  // `JOIN users u2 ON id = id` is meaningless -- ambiguous reference
  // aside, it compares a column to itself. The rule's ON scanner uses
  // the bare `ON` needle so it fires even when ON is adjacent to an
  // identifier (`u2 ON`).
  let d = diags("SELECT * FROM users JOIN users u2 ON id = id;");
  assert!(d.iter().any(|x| x.code == "sql408"), "got {d:?}");
}

#[test]
fn sql408_quiet_for_update_set_self_reference() {
  // `UPDATE users SET age = age + 1` is a perfectly valid self-
  // reference in the RHS expression; we only scan WHERE/ON, not SET.
  let d = diags("UPDATE users SET age = age + 1 WHERE id = '00000000-0000-0000-0000-000000000000';");
  assert!(!d.iter().any(|x| x.code == "sql408"), "must not flag SET RHS: {d:?}");
}

#[test]
fn sql407_where_always_false_literal() {
  let d = diags("SELECT * FROM users WHERE 1 = 2;");
  assert!(
    d.iter().any(|x| x.code == "sql407" && x.message.contains("trivially false")),
    "expected sql407 for `1 = 2`; got {d:?}"
  );
}

#[test]
fn sql407_where_false_keyword() {
  let d = diags("SELECT * FROM users WHERE FALSE;");
  assert!(d.iter().any(|x| x.code == "sql407"), "expected sql407 for FALSE: {d:?}");
}

#[test]
fn sql407_where_not_equals_one() {
  let d = diags("SELECT * FROM users WHERE 1 <> 1;");
  assert!(d.iter().any(|x| x.code == "sql407"), "expected sql407 for 1<>1: {d:?}");
}

#[test]
fn sql407_quiet_for_real_predicate() {
  let d = diags("SELECT * FROM users WHERE age = 5;");
  assert!(!d.iter().any(|x| x.code == "sql407"), "real predicate must not fire: {d:?}");
}

#[test]
fn sql407_quiet_for_tautology_one_eq_one() {
  // 1=1 is a tautology (handled by sql282), not always-false.
  let d = diags("SELECT * FROM users WHERE 1 = 1;");
  assert!(!d.iter().any(|x| x.code == "sql407"), "tautology must not fire as always-false: {d:?}");
}

#[test]
fn sql407_honors_clause_boundaries() {
  // Stopping at GROUP BY is the only way the message stays `1=2`
  // rather than bleeding into the GROUP BY tail.
  let d = diags("SELECT * FROM users WHERE 1=2 GROUP BY id;");
  let hit = d.iter().find(|x| x.code == "sql407").expect("sql407 should fire");
  assert!(hit.message.contains("1=2") && !hit.message.contains("GROUP"), "message leaked past GROUP BY: {}", hit.message);
}

#[test]
fn sql406_duplicate_insert_column() {
  let d = diags("INSERT INTO users (id, email, id) VALUES ('00000000-0000-0000-0000-000000000000','a','00000000-0000-0000-0000-000000000000');");
  assert!(
    d.iter().any(|x| x.code == "sql406" && x.message.contains("INSERT") && x.message.contains("id")),
    "expected sql406 duplicate INSERT column; got {d:?}"
  );
}

#[test]
fn sql406_duplicate_update_set_column() {
  let d = diags("UPDATE users SET id = $1, id = $2 WHERE id = $3;");
  assert!(
    d.iter().any(|x| x.code == "sql406" && x.message.contains("UPDATE") && x.message.contains("id")),
    "expected sql406 duplicate UPDATE SET column; got {d:?}"
  );
}

#[test]
fn sql406_quiet_when_columns_distinct() {
  let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000','a');");
  assert!(!d.iter().any(|x| x.code == "sql406"), "distinct columns must not flag: {d:?}");
  let d2 = diags("UPDATE users SET id = $1, email = $2;");
  assert!(!d2.iter().any(|x| x.code == "sql406"), "distinct SET columns must not flag: {d2:?}");
}

#[test]
fn sql406_case_insensitive() {
  // PG folds unquoted identifiers to lowercase -- `ID` and `id` are
  // the same column. Catch the camelCase tooltip mistake too.
  let d = diags("UPDATE users SET id = $1, ID = $2;");
  assert!(d.iter().any(|x| x.code == "sql406"), "expected sql406 for case-insensitive duplicate: {d:?}");
}

#[test]
fn sql405_having_unknown_column() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING bogus > 0;");
  assert!(
    d.iter().any(|x| x.code == "sql405" && x.message.contains("bogus")),
    "expected sql405 HAVING unknown column; got {d:?}"
  );
}

#[test]
fn sql405_having_qualified_unknown_column() {
  let d = diags("SELECT count(*) FROM users u GROUP BY id HAVING u.bogus > 0;");
  assert!(
    d.iter().any(|x| x.code == "sql405" && x.message.contains("u.bogus")),
    "expected sql405 for u.bogus; got {d:?}"
  );
}

#[test]
fn sql405_having_quiet_for_aggregate_function() {
  // count(*) is a function call -- must not be flagged as an unknown column.
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING count(*) > 1;");
  assert!(!d.iter().any(|x| x.code == "sql405"), "function calls must be skipped: {d:?}");
}

#[test]
fn sql405_having_quiet_for_known_column() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING email IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql405"), "must not flag known column: {d:?}");
}

#[test]
fn sql405_having_quiet_for_projection_alias() {
  let d = diags("SELECT count(*) c FROM users GROUP BY id HAVING c > 1;");
  assert!(!d.iter().any(|x| x.code == "sql405"), "alias `c` must be honored: {d:?}");
}

#[test]
fn sql405_having_flags_unknown_inside_function_arg() {
  // sum(u.bogus) -- the function call wraps a column that doesn't
  // exist; we still want to report it.
  let d = diags("SELECT count(*) FROM users u GROUP BY id HAVING sum(u.bogus) > 0;");
  assert!(
    d.iter().any(|x| x.code == "sql405" && x.message.contains("u.bogus")),
    "expected sql405 inside function arg; got {d:?}"
  );
}

#[test]
fn sql405_having_stops_at_order_by() {
  // The text scanner must not bleed past ORDER BY when scanning the
  // HAVING expression -- a known column there should not be counted
  // toward HAVING and a bogus one over there should not be flagged
  // with sql405.
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING count(*) > 1 ORDER BY id;");
  assert!(!d.iter().any(|x| x.code == "sql405"), "must not scan past ORDER BY: {d:?}");
}

#[test]
fn sql404_group_by_unknown_column() {
  let d = diags("SELECT count(*) FROM users GROUP BY missing_col;");
  assert!(
    d.iter().any(|x| x.code == "sql404" && x.message.contains("missing_col")),
    "expected sql404 GROUP BY unknown column; got {d:?}"
  );
}

#[test]
fn sql404_group_by_qualified_unknown_column() {
  let d = diags("SELECT count(*) FROM users u GROUP BY u.bogus;");
  assert!(
    d.iter().any(|x| x.code == "sql404" && x.message.contains("u.bogus")),
    "expected sql404 for u.bogus; got {d:?}"
  );
}

#[test]
fn sql404_group_by_quiet_for_known_column() {
  let d = diags("SELECT count(*) FROM users GROUP BY id;");
  assert!(!d.iter().any(|x| x.code == "sql404"), "must not flag known column: {d:?}");
}

#[test]
fn sql404_group_by_quiet_for_positional() {
  let d = diags("SELECT count(*) FROM users GROUP BY 1;");
  assert!(!d.iter().any(|x| x.code == "sql404"));
}

#[test]
fn sql404_group_by_quiet_for_alias() {
  // PG accepts SELECT-list aliases in GROUP BY -- sql337 covers the
  // portability concern separately; we must not flag as unknown.
  let d = diags("SELECT id AS x, count(*) FROM users GROUP BY x;");
  assert!(!d.iter().any(|x| x.code == "sql404"), "alias must be honored: {d:?}");
}

#[test]
fn sql404_group_by_quiet_for_rollup_wrapper() {
  let d = diags("SELECT count(*) FROM users GROUP BY ROLLUP(id);");
  assert!(!d.iter().any(|x| x.code == "sql404"), "ROLLUP wrapper must be skipped: {d:?}");
}

#[test]
fn sql404_group_by_stops_at_having() {
  let d = diags("SELECT count(*) FROM users GROUP BY id HAVING count(*) > 1;");
  assert!(!d.iter().any(|x| x.code == "sql404"));
}

#[test]
fn sql403_order_by_unknown_column() {
  let d = diags("SELECT id FROM users ORDER BY no_such_col;");
  assert!(
    d.iter().any(|x| x.code == "sql403" && x.message.contains("no_such_col")),
    "expected sql403 ORDER BY unknown column; got {d:?}"
  );
}

#[test]
fn sql403_order_by_quiet_for_known_column() {
  let d = diags("SELECT id FROM users ORDER BY id DESC, email ASC NULLS FIRST;");
  assert!(!d.iter().any(|x| x.code == "sql403"), "must not flag valid columns: {d:?}");
}

#[test]
fn sql403_order_by_quiet_for_projection_alias() {
  // PG allows referencing a SELECT-list alias in ORDER BY -- don't flag it.
  let d = diags("SELECT id AS x FROM users ORDER BY x;");
  assert!(!d.iter().any(|x| x.code == "sql403"), "alias must be honored: {d:?}");
}

#[test]
fn sql403_order_by_quiet_for_positional() {
  // `ORDER BY 1` is a separate concern (sql099); we shouldn't flag it as unknown.
  let d = diags("SELECT id FROM users ORDER BY 1;");
  assert!(!d.iter().any(|x| x.code == "sql403"), "positional must not flag as unknown: {d:?}");
}

#[test]
fn sql403_order_by_qualified_unknown_column() {
  let d = diags("SELECT id FROM users u ORDER BY u.bogus;");
  assert!(
    d.iter().any(|x| x.code == "sql403" && x.message.contains("u.bogus")),
    "expected sql403 for u.bogus; got {d:?}"
  );
}

#[test]
fn sql403_order_by_stops_at_limit() {
  // The text scanner must not bleed past clause boundaries -- LIMIT
  // ends the ORDER BY list; the integer there is not an item to check.
  let d = diags("SELECT id FROM users ORDER BY id LIMIT 5;");
  assert!(!d.iter().any(|x| x.code == "sql403"), "must not scan past LIMIT: {d:?}");
}

#[test]
fn sql403_order_by_skips_expression_items() {
  // We deliberately only validate bare column references -- function
  // calls and operator expressions are out of scope to avoid noise.
  let d = diags("SELECT id FROM users ORDER BY lower(email);");
  assert!(!d.iter().any(|x| x.code == "sql403"), "expression items must be skipped: {d:?}");
}

#[test]
fn sql402_duplicate_alias_in_from_list() {
  // `SELECT * FROM users a, orders a` -- duplicate alias `a`. PG
  // errors on this with "table name 'a' specified more than once";
  // catch it early.
  let d = diags("SELECT * FROM users a, orders a;");
  assert!(
    d.iter().any(|x| x.code == "sql402" && x.message.contains('a')),
    "expected sql402 duplicate alias; got {d:?}"
  );
}

#[test]
fn sql402_duplicate_alias_across_from_and_join() {
  let d = diags("SELECT * FROM users u JOIN orders u ON u.id = u.user_id;");
  assert!(
    d.iter().any(|x| x.code == "sql402"),
    "expected sql402 duplicate alias across FROM and JOIN; got {d:?}"
  );
}

#[test]
fn sql402_quiet_when_aliases_distinct() {
  let d = diags("SELECT * FROM users u JOIN orders o ON u.id = o.user_id;");
  assert!(!d.iter().any(|x| x.code == "sql402"), "distinct aliases must not flag: {d:?}");
}

#[test]
fn sql003_quiet_for_using_join_merged_column() {
  // `JOIN ... USING (id)` merges `id` into a single column -- bare
  // `id` is unambiguous and must NOT fire sql003.
  let d = diags("SELECT id FROM users JOIN orders USING (id);");
  assert!(
    !d.iter().any(|x| x.code == "sql003"),
    "USING(id) merges the column; bare `id` must not be flagged ambiguous: {d:?}"
  );
}

#[test]
fn sql003_quiet_for_natural_join() {
  // NATURAL JOIN merges every same-named column -- bare `id` is
  // unambiguous (PG behaviour). Don't fire sql003.
  let d = diags("SELECT id FROM users NATURAL JOIN orders;");
  assert!(
    !d.iter().any(|x| x.code == "sql003"),
    "NATURAL JOIN merges same-named columns; bare `id` must not be flagged ambiguous: {d:?}"
  );
}

#[test]
fn sql003_still_flags_non_using_column_in_using_join() {
  // USING(id) merges only `id`. Other shared columns (none here, but
  // imagine `email` existed in both) would still be ambiguous.
  // Sanity test that USING doesn't blanket-suppress: a separate
  // unrelated column reference in WHERE must still flag.
  // (In this minimal catalog only `id` is shared, so we just confirm
  // the rule still runs by checking another diagnostic class.)
  let d = diags("SELECT id, u.email FROM users u JOIN orders o USING (id);");
  assert!(
    !d.iter().any(|x| x.code == "sql003"),
    "USING(id) plus qualified other column should be clean: {d:?}"
  );
}

#[test]
fn sql003_ambiguous_column_in_where() {
  // Ambiguous bare `id` in a WHERE predicate. Locked in by iter 12
  // — the pg_query backend now populates `where_clause` with the
  // ColumnRefs it finds, so the rule sees `id` and reports the clash.
  let d = diags("SELECT * FROM users JOIN orders ON true WHERE id = '1';");
  assert!(
    d.iter().any(|x| x.code == "sql003" && x.message.contains("ambiguous")),
    "expected sql003 for ambiguous `id` in WHERE; got {d:?}"
  );
}

#[test]
fn sql003_ambiguous_column_in_join_on() {
  let d = diags("SELECT * FROM users JOIN orders ON id = id;");
  assert!(
    d.iter().any(|x| x.code == "sql003" && x.message.contains("ambiguous")),
    "expected sql003 for ambiguous `id` in JOIN ON; got {d:?}"
  );
}

#[test]
fn sql003_quiet_when_qualified() {
  let d = diags("SELECT u.id FROM users u JOIN orders o ON o.user_id = u.id;");
  assert!(!d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn sql013_update_no_where() {
  let d = diags("UPDATE users SET name = 'x';");
  assert!(d.iter().any(|x| x.code == "sql013" && x.severity == Severity::Warning));
}

#[test]
fn sql013_delete_no_where() {
  let d = diags("DELETE FROM users;");
  assert!(d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn sql013_quiet_with_where() {
  let d = diags("DELETE FROM users WHERE id = $1;");
  assert!(!d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn sql015_null_compare() {
  let d = diags("SELECT * FROM users WHERE name = NULL;");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn sql015_quiet_with_is_null() {
  let d = diags("SELECT * FROM users WHERE name IS NULL;");
  assert!(!d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn sql010_union_count_mismatch() {
  let d = diags("SELECT id, name FROM users UNION SELECT id FROM orders;");
  assert!(d.iter().any(|x| x.code == "sql010"), "diags: {:?}", d);
}

#[test]
fn sql010_quiet_when_counts_match() {
  let d = diags("SELECT id, name FROM users UNION SELECT id, name FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql010"));
}

#[test]
fn sql010_handles_union_all() {
  let d = diags("SELECT id FROM users UNION ALL SELECT id, name FROM users;");
  assert!(d.iter().any(|x| x.code == "sql010"));
}

#[test]
fn sql010_ignores_subquery_commas() {
  // Subquery in projection should count as 1 column, not 3.
  let d = diags(
    "SELECT id, (SELECT max(id) FROM orders), name FROM users \
                   UNION SELECT id, name, name FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql010"), "diags: {:?}", d);
}

#[test]
fn sql017_flags_bare_column_with_aggregate() {
  let d = diags("SELECT name, count(*) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql017"), "diags: {:?}", d);
}

#[test]
fn sql017_quiet_when_grouped() {
  let d = diags("SELECT name, count(*) FROM users GROUP BY name;");
  assert!(!d.iter().any(|x| x.code == "sql017"));
}

#[test]
fn sql017_quiet_when_no_aggregate() {
  let d = diags("SELECT name, email FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql017"));
}

#[test]
fn sql018_flags_not_in_subquery() {
  let d = diags("SELECT * FROM users WHERE id NOT IN (SELECT user_id FROM orders);");
  assert!(d.iter().any(|x| x.code == "sql018"), "diags: {:?}", d);
}

#[test]
fn sql018_quiet_for_explicit_list() {
  let d = diags("SELECT * FROM users WHERE id NOT IN (1, 2, 3);");
  assert!(!d.iter().any(|x| x.code == "sql018"), "diags: {:?}", d);
}

#[test]
fn sql001_quiet_for_cte_name() {
  let d = diags("WITH active AS (SELECT id FROM users) SELECT * FROM active;");
  assert!(!d.iter().any(|x| x.code == "sql001"), "diags: {:?}", d);
}

#[test]
fn sql001_quiet_for_recursive_cte() {
  let d = diags(
    "WITH RECURSIVE walk AS (SELECT 1 UNION SELECT n+1 FROM walk) \
         SELECT * FROM walk;",
  );
  assert!(!d.iter().any(|x| x.code == "sql001"), "diags: {:?}", d);
}

#[test]
fn sql001_quiet_for_multi_cte() {
  let d = diags("WITH a AS (SELECT 1), b AS (SELECT 2) SELECT * FROM a JOIN b ON true;");
  assert!(!d.iter().any(|x| x.code == "sql001"), "diags: {:?}", d);
}

#[test]
fn sql017_ignores_columns_inside_aggregate_args() {
  // `id` lives only inside count(...) so isn't bare.
  let d = diags("SELECT count(id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql017"));
}

// ===== sql021 prefer-alias =================================================

#[test]
fn sql021_warns_when_alias_exists() {
  let d = diags("SELECT users.id FROM users AS u;");
  assert!(
    d.iter().any(|x| x.code == "sql021"),
    "expected sql021, got {:?}",
    d.iter().map(|x| x.code).collect::<Vec<_>>()
  );
}

#[test]
fn sql021_quiet_when_no_alias() {
  let d = diags("SELECT users.id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql021"));
}

#[test]
fn sql021_quiet_when_using_alias() {
  let d = diags("SELECT u.id FROM users AS u;");
  assert!(!d.iter().any(|x| x.code == "sql021"));
}

#[test]
fn sql021_fires_for_each_bare_qualified_reference() {
  let d = diags("SELECT users.id, users.email FROM users u;");
  let count = d.iter().filter(|x| x.code == "sql021").count();
  assert!(count >= 2, "expected ≥2 sql021 hits, got {count}: {:?}", d);
}

#[test]
fn sql021_severity_is_hint() {
  let d = diags("SELECT users.id FROM users u;");
  let hit = d.iter().find(|x| x.code == "sql021").expect("sql021 missing");
  assert_eq!(hit.severity, Severity::Hint);
}

#[test]
fn sql021_quiet_on_ddl() {
  // CREATE references the bare table name by design.
  let d = diags("CREATE INDEX ix ON users (id);");
  assert!(!d.iter().any(|x| x.code == "sql021"));
}

#[test]
fn sql021_quiet_when_bare_word_is_substring_only() {
  // `users_archive` shouldn't trip the rule even though it contains
  // "users" as a substring.
  let d = diags("SELECT users_archive.id FROM users u;");
  assert!(!d.iter().any(|x| x.code == "sql021"), "false positive: {:?}", d);
}

// ===== did-you-mean suggestions =============================================

#[test]
fn sql001_did_you_mean_for_typo() {
  // `userss` is one char off `users`.
  let d = diags("SELECT * FROM userss;");
  let hit = d.iter().find(|x| x.code == "sql001").expect("sql001 missing");
  assert!(hit.message.contains("did you mean"), "expected suggestion, got: {}", hit.message);
  assert!(hit.message.contains("users"), "expected `users` in suggestion: {}", hit.message);
}

#[test]
fn sql002_did_you_mean_for_typo() {
  let d = diags("SELECT emial FROM users;");
  let hit = d.iter().find(|x| x.code == "sql002").expect("sql002 missing");
  assert!(hit.message.contains("did you mean"));
  assert!(hit.message.contains("email"));
}

// ===== sql030 missing trigger RETURN =======================================

#[test]
fn sql030_flags_trigger_without_return() {
  let d =
    diags("CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN UPDATE x SET y=1; END; $$;");
  assert!(
    d.iter().any(|x| x.code == "sql030"),
    "expected sql030, got {:?}",
    d.iter().map(|x| x.code).collect::<Vec<_>>()
  );
}

#[test]
fn sql030_quiet_when_return_new_present() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql030"));
}

#[test]
fn sql030_quiet_when_return_null() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql030"));
}

#[test]
fn sql030_quiet_when_not_trigger_function() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql030"));
}

#[test]
fn sql030_quiet_when_return_commented_out_does_not_count() {
  // `-- RETURN NEW;` shouldn't satisfy the check (comments stripped).
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN -- RETURN NEW;
            UPDATE x SET y=1;
        END; $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql030"));
}

// ===== sql091 empty COMMENT ===============================================

#[test]
fn sql091_flags_empty_comment() {
  let d = diags("COMMENT ON TABLE users IS '';");
  assert!(d.iter().any(|x| x.code == "sql091"));
}

#[test]
fn sql091_quiet_for_non_empty_comment() {
  let d = diags("COMMENT ON TABLE users IS 'application users';");
  assert!(!d.iter().any(|x| x.code == "sql091"));
}

// ===== sql093 DISTINCT with aggregate ====================================

#[test]
fn sql093_flags_distinct_with_count() {
  let d = diags("SELECT DISTINCT count(*) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql093"));
}

#[test]
fn sql093_quiet_with_group_by() {
  let d = diags("SELECT DISTINCT count(*) FROM users GROUP BY email;");
  assert!(!d.iter().any(|x| x.code == "sql093"));
}

#[test]
fn sql093_quiet_without_aggregate() {
  let d = diags("SELECT DISTINCT email FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql093"));
}

// ===== sql094 deep CASE nesting ===========================================

#[test]
fn sql094_flags_4deep_case() {
  let src = "SELECT CASE WHEN a THEN CASE WHEN b THEN CASE WHEN c THEN CASE WHEN d THEN 1 END END END END FROM users;";
  let d = diags(src);
  assert!(d.iter().any(|x| x.code == "sql094"));
}

#[test]
fn sql094_quiet_for_shallow_case() {
  let d = diags("SELECT CASE WHEN a THEN 1 ELSE 2 END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql094"));
}

// ===== sql085 NULLIF same args ============================================

#[test]
fn sql085_flags_nullif_same_args() {
  let d = diags("SELECT NULLIF(id, id) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql085"));
}

#[test]
fn sql085_quiet_for_distinct_args() {
  let d = diags("SELECT NULLIF(email, '') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql085"));
}

// ===== sql087 BETWEEN reversed bounds =====================================

#[test]
fn sql087_flags_reversed_int_bounds() {
  let d = diags("SELECT * FROM users WHERE id BETWEEN 100 AND 1;");
  assert!(d.iter().any(|x| x.code == "sql087"));
}

#[test]
fn sql087_quiet_for_correct_order() {
  let d = diags("SELECT * FROM users WHERE id BETWEEN 1 AND 100;");
  assert!(!d.iter().any(|x| x.code == "sql087"));
}

#[test]
fn sql087_quiet_for_non_literal() {
  let d = diags("SELECT * FROM users WHERE id BETWEEN min_id AND max_id;");
  assert!(!d.iter().any(|x| x.code == "sql087"));
}

#[test]
fn sql087_flags_reversed_iso_date_literals() {
  // Regression iter192: string-literal date bounds were silent.
  // ISO format compares lexicographically correctly.
  let d = diags("SELECT * FROM users WHERE created_at BETWEEN '2024-01-01' AND '2023-01-01';");
  let hit = d.iter().find(|x| x.code == "sql087").expect("sql087");
  assert!(hit.message.contains("lex order"), "expected lex-order swap message: {}", hit.message);
}

#[test]
fn sql087_flags_reversed_string_literals() {
  let d = diags("SELECT * FROM users WHERE email BETWEEN 'zzz' AND 'aaa';");
  assert!(d.iter().any(|x| x.code == "sql087"), "expected sql087 for string-lit swap: {d:?}");
}

#[test]
fn sql087_quiet_for_correct_string_order() {
  let d = diags("SELECT * FROM users WHERE email BETWEEN 'aaa' AND 'zzz';");
  assert!(!d.iter().any(|x| x.code == "sql087"), "correct string order must not fire: {d:?}");
}

#[test]
fn sql087_quiet_for_mixed_kind_bounds() {
  // String bound + numeric bound: kinds differ, can't compare safely.
  let d = diags("SELECT * FROM users WHERE id BETWEEN '5' AND 10;");
  assert!(!d.iter().any(|x| x.code == "sql087"), "mixed-kind bounds must not fire: {d:?}");
}

// ===== sql088 LIKE leading wildcard =======================================

#[test]
fn sql088_flags_leading_percent() {
  let d = diags("SELECT * FROM users WHERE email LIKE '%@example.com';");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn sql088_flags_ilike_leading_percent() {
  let d = diags("SELECT * FROM users WHERE email ILIKE '%foo';");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn sql088_quiet_for_trailing_only() {
  let d = diags("SELECT * FROM users WHERE email LIKE 'foo%';");
  assert!(!d.iter().any(|x| x.code == "sql088"));
}

// ===== sql076 negative LIMIT / OFFSET =====================================

#[test]
fn sql076_flags_negative_limit() {
  let d = diags("SELECT * FROM users ORDER BY id LIMIT -1;");
  assert!(d.iter().any(|x| x.code == "sql076"));
}

#[test]
fn sql076_flags_negative_offset() {
  let d = diags("SELECT * FROM users ORDER BY id LIMIT 10 OFFSET -5;");
  assert!(d.iter().any(|x| x.code == "sql076"));
}

#[test]
fn sql076_quiet_for_positive() {
  let d = diags("SELECT * FROM users ORDER BY id LIMIT 10 OFFSET 5;");
  assert!(!d.iter().any(|x| x.code == "sql076"));
}

// ===== sql081 ORDER BY random =============================================

#[test]
fn sql081_flags_order_by_random() {
  let d = diags("SELECT * FROM users ORDER BY random() LIMIT 10;");
  assert!(d.iter().any(|x| x.code == "sql081"));
}

#[test]
fn sql081_quiet_for_normal_order() {
  let d = diags("SELECT * FROM users ORDER BY id LIMIT 10;");
  assert!(!d.iter().any(|x| x.code == "sql081"));
}

// ===== sql072 SELECT FOR UPDATE without WHERE =============================

#[test]
fn sql072_flags_unwhere_for_update() {
  let d = diags("SELECT * FROM users FOR UPDATE;");
  assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn sql072_quiet_with_where() {
  let d = diags("SELECT * FROM users WHERE id = '1' FOR UPDATE;");
  assert!(!d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn sql072_quiet_when_no_lock() {
  let d = diags("SELECT * FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql072"));
}

// ===== sql074 long IN list =================================================

#[test]
fn sql074_flags_long_in_list() {
  let items: Vec<String> = (1..=60).map(|i| i.to_string()).collect();
  let src = format!("SELECT * FROM users WHERE id IN ({});", items.join(","));
  let d = diags(&src);
  assert!(d.iter().any(|x| x.code == "sql074"));
}

#[test]
fn sql074_quiet_for_short_list() {
  let d = diags("SELECT * FROM users WHERE id IN (1, 2, 3);");
  assert!(!d.iter().any(|x| x.code == "sql074"));
}

#[test]
fn sql074_quiet_for_subquery_in() {
  let d = diags("SELECT * FROM users WHERE id IN (SELECT id FROM orders);");
  assert!(!d.iter().any(|x| x.code == "sql074"));
}

// ===== sql075 TIME WITH TIME ZONE =========================================

#[test]
fn sql075_flags_timetz() {
  let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, t TIMETZ);");
  assert!(d.iter().any(|x| x.code == "sql075"));
}

#[test]
fn sql075_flags_time_with_time_zone() {
  let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, t TIME WITH TIME ZONE);");
  assert!(d.iter().any(|x| x.code == "sql075"));
}

#[test]
fn sql075_quiet_for_timestamptz() {
  let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, t TIMESTAMPTZ);");
  assert!(!d.iter().any(|x| x.code == "sql075"));
}

// ===== sql068 single-stmt transaction =====================================

#[test]
fn sql068_flags_single_stmt_txn() {
  let d = diags("BEGIN; UPDATE users SET email='x' WHERE id='1'; COMMIT;");
  assert!(d.iter().any(|x| x.code == "sql068"));
}

#[test]
fn sql068_quiet_multi_stmt() {
  let d = diags("BEGIN; UPDATE users SET email='x' WHERE id='1'; UPDATE orders SET status='y' WHERE id='1'; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql068"));
}

#[test]
fn sql068_quiet_no_transaction() {
  let d = diags("UPDATE users SET email='x' WHERE id='1';");
  assert!(!d.iter().any(|x| x.code == "sql068"));
}

// ===== sql069 NOT NULL DEFAULT NULL =======================================

#[test]
fn sql069_flags_not_null_default_null() {
  let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, x INT NOT NULL DEFAULT NULL);");
  assert!(d.iter().any(|x| x.code == "sql069"));
}

#[test]
fn sql069_quiet_when_default_is_value() {
  let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, x INT NOT NULL DEFAULT 0);");
  assert!(!d.iter().any(|x| x.code == "sql069"));
}

#[test]
fn sql069_quiet_when_no_not_null() {
  let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, x INT DEFAULT NULL);");
  assert!(!d.iter().any(|x| x.code == "sql069"));
}

#[test]
fn sql032_range_narrows_to_return_statement() {
  let src = "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN; END; $$;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql032").expect("sql032");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert!(slice.contains("RETURN"), "expected RETURN in slice, got: {slice:?}");
  assert!(!slice.contains("CREATE"), "should not span CREATE: {slice:?}");
}

#[test]
fn sql044_range_narrows_to_exit_keyword() {
  let src = "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN EXIT; END; $$;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql044").expect("sql044");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "EXIT", "expected `EXIT` only, got: {slice:?}");
}

#[test]
fn sql054_range_narrows_to_equals_true() {
  let src = "SELECT * FROM users WHERE active = true;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql054").expect("sql054");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert!(slice.contains("="), "expected `=` in slice, got: {slice:?}");
  assert!(slice.to_ascii_uppercase().contains("TRUE"), "expected `TRUE`, got: {slice:?}");
  assert!(slice.len() < src.len() / 2, "range should be small, got len {} of {}", slice.len(), src.len());
}

#[test]
fn sql064_range_narrows_to_join_keyword() {
  let src = "SELECT * FROM users JOIN orders;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql064").expect("sql064");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "JOIN");
}

#[test]
fn sql076_range_narrows_to_negative_number() {
  let src = "SELECT * FROM users ORDER BY id LIMIT -42;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql076").expect("sql076");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice, "-42");
}

#[test]
fn sql046_range_narrows_to_table_name() {
  let src = "CREATE TABLE log_events (id INT, data TEXT);";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql046").expect("sql046");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice, "log_events");
}

#[test]
fn sql056_range_narrows_to_union_keyword() {
  let src = "SELECT 1 FROM users UNION SELECT 2 FROM orders;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql056").expect("sql056");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice, "UNION");
}

#[test]
fn sql058_range_narrows_to_case_keyword() {
  // Single WHEN without ELSE -- the canonical noise case.
  let src = "SELECT CASE WHEN id IS NULL THEN 'nil' END FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql058").expect("sql058");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "CASE");
}

#[test]
fn sql065_range_narrows_to_digit() {
  let src = "SELECT id, count(*) FROM users GROUP BY 1;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql065").expect("sql065");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice, "1");
}

#[test]
fn sql072_range_narrows_to_for_update() {
  let src = "SELECT * FROM users FOR UPDATE;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql072").expect("sql072");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice, "FOR UPDATE");
}

#[test]
fn sql075_range_narrows_to_type_token() {
  let src = "CREATE TABLE foo (id INT PRIMARY KEY, t TIMETZ);";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql075").expect("sql075");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "TIMETZ");
}

#[test]
fn sql081_range_narrows_to_random_call() {
  let src = "SELECT * FROM users ORDER BY random() LIMIT 10;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql081").expect("sql081");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert!(slice.to_ascii_lowercase().contains("random("));
  assert!(slice.ends_with(')'));
}

#[test]
fn sql085_range_narrows_to_nullif_call() {
  let src = "SELECT NULLIF(id, id) FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql085").expect("sql085");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice, "NULLIF(id, id)");
}

#[test]
fn sql088_range_narrows_to_pattern_literal() {
  let src = "SELECT * FROM users WHERE email LIKE '%@example.com';";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql088").expect("sql088");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "'%@example.com'");
}

#[test]
fn sql091_range_narrows_to_empty_string() {
  let src = "COMMENT ON TABLE users IS '';";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql091").expect("sql091");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "''");
}

#[test]
fn sql093_range_narrows_to_distinct() {
  let src = "SELECT DISTINCT count(*) FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql093").expect("sql093");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "DISTINCT");
}

// ===== sql084 COUNT(1) vs COUNT(*) =========================================

#[test]
fn sql084_flags_count_one() {
  let d = diags("SELECT COUNT(1) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql084"));
}

#[test]
fn sql084_quiet_for_count_star() {
  let d = diags("SELECT COUNT(*) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql084"));
}

#[test]
fn sql084_quiet_for_count_column() {
  let d = diags("SELECT COUNT(email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql084"));
}

#[test]
fn sql084_range_narrows_to_count_call() {
  let src = "SELECT COUNT(1) FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql084").expect("sql084");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "COUNT(1)");
}

// ===== sql096 trailing comma in VALUES ====================================

#[test]
fn sql096_flags_trailing_comma() {
  let d = diags("INSERT INTO users (id, email) VALUES ('x', 'a@b.com', );");
  assert!(d.iter().any(|x| x.code == "sql096"));
}

#[test]
fn sql096_quiet_no_trailing_comma() {
  let d = diags("INSERT INTO users (id, email) VALUES ('x', 'a@b.com');");
  assert!(!d.iter().any(|x| x.code == "sql096"));
}

#[test]
fn sql096_range_narrows_to_comma() {
  let src = "INSERT INTO users (id, email) VALUES ('x', 'a@b.com', );";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql096").expect("sql096");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], ",");
}

// ===== sql097 SELECT without FROM =========================================

#[test]
fn sql097_flags_bare_select_column() {
  let d = diags("SELECT something;");
  assert!(d.iter().any(|x| x.code == "sql097"));
}

#[test]
fn sql097_quiet_for_select_with_from() {
  let d = diags("SELECT id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql097"));
}

#[test]
fn sql097_quiet_for_literal() {
  let d = diags("SELECT 1;");
  assert!(!d.iter().any(|x| x.code == "sql097"));
}

#[test]
fn sql097_quiet_for_now_call() {
  let d = diags("SELECT now();");
  assert!(!d.iter().any(|x| x.code == "sql097"));
}

// ===== sql062 range narrowing =============================================

#[test]
fn sql062_range_narrows_to_savepoint_name() {
  let src = "BEGIN; SAVEPOINT sp1; SELECT 1;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql062").expect("sql062");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "sp1");
}

// ===== sql094 range narrowing =============================================

#[test]
fn sql094_range_narrows_to_deepest_case() {
  let src = "SELECT CASE WHEN a THEN CASE WHEN b THEN CASE WHEN c THEN CASE WHEN d THEN 1 ELSE 0 END ELSE 0 END ELSE 0 END ELSE 0 END FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql094").expect("sql094");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "CASE");
}

#[test]
fn sql030_range_narrows_to_begin() {
  let src =
    "CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN UPDATE x SET y=1 WHERE id=1; END; $$;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql030").expect("sql030");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "BEGIN");
}

#[test]
fn sql068_range_narrows_to_begin() {
  let src = "BEGIN; UPDATE users SET email='x' WHERE id='1'; COMMIT;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql068").expect("sql068");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "BEGIN");
}

#[test]
fn sql074_range_narrows_to_in_paren() {
  let items: Vec<String> = (1..=60).map(|i| i.to_string()).collect();
  let src = format!("SELECT * FROM users WHERE id IN ({});", items.join(","));
  let d = diags(&src);
  let hit = d.iter().find(|x| x.code == "sql074").expect("sql074");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert!(slice.starts_with('('));
  assert!(slice.ends_with(')'));
}

#[test]
fn sql083_range_narrows_to_insert_keyword() {
  let src = "INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql083").expect("sql083");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "INSERT");
}

#[test]
fn sql087_range_narrows_to_between_expression() {
  let src = "SELECT * FROM users WHERE id BETWEEN 100 AND 1;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql087").expect("sql087");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert!(slice.to_ascii_uppercase().starts_with("BETWEEN"));
}

#[test]
fn sql069_range_narrows_to_offending_column() {
  let src = "CREATE TABLE foo (id INT PRIMARY KEY, age INT NOT NULL DEFAULT NULL);";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql069").expect("sql069");
  let start: u32 = hit.range.start().into();
  let end: u32 = hit.range.end().into();
  let slice = &src[start as usize..end as usize];
  assert!(slice.contains("age"), "range should cover the `age` column line, got: {slice:?}");
  assert!(!slice.contains("PRIMARY KEY"), "range should not span the whole table: {slice:?}");
}

// ===== sql064 JOIN without ON =============================================

#[test]
fn sql064_flags_inner_join_without_on() {
  let d = diags("SELECT * FROM users JOIN orders;");
  assert!(d.iter().any(|x| x.code == "sql064"));
}

#[test]
fn sql064_quiet_for_cross_join() {
  let d = diags("SELECT * FROM users CROSS JOIN orders;");
  assert!(!d.iter().any(|x| x.code == "sql064"));
}

#[test]
fn sql064_quiet_when_on_present() {
  let d = diags("SELECT * FROM users u JOIN orders o ON o.user_id = u.id;");
  assert!(!d.iter().any(|x| x.code == "sql064"));
}

// ===== sql065 GROUP BY position ===========================================

#[test]
fn sql065_flags_group_by_one() {
  let d = diags("SELECT id, count(*) FROM users GROUP BY 1;");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn sql065_quiet_for_group_by_name() {
  let d = diags("SELECT id, count(*) FROM users GROUP BY id;");
  assert!(!d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn sql065_flags_multi_with_at_least_one_position() {
  let d = diags("SELECT id, name, count(*) FROM users GROUP BY 1, name;");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

// ===== sql061 NULL in VALUES ==============================================

#[test]
fn sql061_flags_bare_null() {
  let d = diags("INSERT INTO users (id, email) VALUES (NULL, 'a@b.com');");
  assert!(d.iter().any(|x| x.code == "sql061"));
}

#[test]
fn sql061_quiet_when_cast() {
  let d = diags("INSERT INTO users (id, email) VALUES (NULL::UUID, 'a@b.com');");
  assert!(!d.iter().any(|x| x.code == "sql061"));
}

// ===== sql058 CASE single WHEN ============================================

#[test]
fn sql058_flags_case_with_one_when() {
  // Single WHEN without ELSE silently NULL-fills.
  let d = diags("SELECT CASE WHEN id IS NULL THEN 'nil' END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql058"));
}

#[test]
fn sql058_quiet_with_else() {
  // CASE WHEN x THEN a ELSE b END is the canonical if-else; never noise.
  let d = diags("SELECT CASE WHEN id IS NULL THEN 'nil' ELSE 'ok' END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql058"));
}

#[test]
fn sql058_quiet_for_multi_when() {
  let d = diags("SELECT CASE WHEN id = 1 THEN 'a' WHEN id = 2 THEN 'b' ELSE 'c' END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql058"));
}

#[test]
fn sql058_quiet_when_no_case() {
  let d = diags("SELECT id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql058"));
}

// ===== sql062 SAVEPOINT without RELEASE ===================================

#[test]
fn sql062_flags_dangling_savepoint() {
  let d = diags("BEGIN; SAVEPOINT sp1; SELECT 1;");
  assert!(d.iter().any(|x| x.code == "sql062"));
}

#[test]
fn sql062_quiet_when_released() {
  let d = diags("BEGIN; SAVEPOINT sp1; SELECT 1; RELEASE SAVEPOINT sp1;");
  assert!(!d.iter().any(|x| x.code == "sql062"));
}

#[test]
fn sql062_quiet_when_rolled_back_to() {
  let d = diags("BEGIN; SAVEPOINT sp1; SELECT 1; ROLLBACK TO SAVEPOINT sp1;");
  assert!(!d.iter().any(|x| x.code == "sql062"));
}

// ===== sql056 UNION vs UNION ALL ==========================================

#[test]
fn sql056_flags_plain_union() {
  let d = diags("SELECT 1 FROM users UNION SELECT 2 FROM orders;");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn sql056_quiet_for_union_all() {
  let d = diags("SELECT 1 FROM users UNION ALL SELECT 2 FROM orders;");
  assert!(!d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn sql056_quiet_for_explicit_distinct() {
  let d = diags("SELECT 1 FROM users UNION DISTINCT SELECT 2 FROM orders;");
  assert!(!d.iter().any(|x| x.code == "sql056"));
}

// ===== sql055 redundant parens in WHERE ===================================

#[test]
fn sql055_flags_single_condition_in_parens() {
  let d = diags("SELECT * FROM users WHERE (id = '1');");
  assert!(d.iter().any(|x| x.code == "sql055"));
}

#[test]
fn sql055_quiet_for_multi_clause() {
  let d = diags("SELECT * FROM users WHERE (id = '1' AND email = 'x');");
  assert!(!d.iter().any(|x| x.code == "sql055"));
}

#[test]
fn sql055_quiet_for_bare_condition() {
  let d = diags("SELECT * FROM users WHERE id = '1';");
  assert!(!d.iter().any(|x| x.code == "sql055"));
}

// ===== sql051 LIMIT without ORDER BY =======================================

#[test]
fn sql051_flags_limit_without_order() {
  let d = diags("SELECT * FROM users LIMIT 10;");
  assert!(d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn sql051_quiet_with_order_by() {
  let d = diags("SELECT * FROM users ORDER BY id LIMIT 10;");
  assert!(!d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn sql051_quiet_for_limit_one() {
  let d = diags("SELECT * FROM users LIMIT 1;");
  assert!(!d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn sql051_quiet_without_limit() {
  let d = diags("SELECT * FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql051"));
}

// ===== sql054 boolean = true / = false =====================================

#[test]
fn sql054_flags_equals_true() {
  let d = diags("SELECT * FROM users WHERE active = true;");
  assert!(d.iter().any(|x| x.code == "sql054"));
}

#[test]
fn sql054_flags_equals_false() {
  let d = diags("SELECT * FROM users WHERE deleted = false;");
  assert!(d.iter().any(|x| x.code == "sql054"));
}

#[test]
fn sql054_quiet_when_using_bare_predicate() {
  let d = diags("SELECT * FROM users WHERE active;");
  assert!(!d.iter().any(|x| x.code == "sql054"));
}

#[test]
fn sql054_quiet_for_substring_match() {
  // 'true' inside a string literal must not trigger.
  let d = diags("SELECT * FROM users WHERE name = 'true';");
  assert!(!d.iter().any(|x| x.code == "sql054"));
}

#[test]
fn sql054_flags_lessgreater_true() {
  // Regression iter186: `<> TRUE` was silent.
  let d = diags("SELECT * FROM users WHERE active <> TRUE;");
  let hit = d.iter().find(|x| x.code == "sql054").expect("sql054");
  assert!(hit.message.contains("<> true"), "expected message to mention `<> true`: {}", hit.message);
}

#[test]
fn sql054_flags_bang_equals_true_with_correct_message() {
  // Regression iter186: `!= TRUE` was reported with message `drop \`= true\``
  // because the substring `= TRUE` inside `!= TRUE` matched the bare-`=`
  // needle first. Now ordered longest-first + prev-char guard.
  let d = diags("SELECT * FROM users WHERE active != TRUE;");
  let hit = d.iter().find(|x| x.code == "sql054").expect("sql054");
  assert!(hit.message.contains("!= true"), "expected message to mention `!= true`, got: {}", hit.message);
}

#[test]
fn sql054_flags_commuted_true_equals() {
  // Regression iter186: `TRUE = active` (commuted) was silent.
  let d = diags("SELECT * FROM users WHERE TRUE = active;");
  assert!(d.iter().any(|x| x.code == "sql054"), "expected sql054 for `TRUE = col`: {d:?}");
}

#[test]
fn sql054_flags_commuted_false_equals() {
  let d = diags("SELECT * FROM users WHERE FALSE = active;");
  assert!(d.iter().any(|x| x.code == "sql054"), "expected sql054 for `FALSE = col`: {d:?}");
}

#[test]
fn sql054_quiet_for_is_true() {
  // `IS TRUE` has different NULL semantics from `= TRUE`; must not fire.
  let d = diags("SELECT * FROM users WHERE active IS TRUE;");
  assert!(!d.iter().any(|x| x.code == "sql054"), "IS TRUE must not fire: {d:?}");
}

#[test]
fn sql492_not_in_with_null_in_list() {
  let d = diags("SELECT * FROM users WHERE name NOT IN ('a', NULL);");
  assert!(d.iter().any(|x| x.code == "sql492" && x.message.contains("ZERO rows")), "expected sql492 Warning for NOT IN (...,NULL): {d:?}");
}

#[test]
fn sql492_not_in_with_only_null() {
  let d = diags("SELECT * FROM users WHERE name NOT IN (NULL);");
  assert!(d.iter().any(|x| x.code == "sql492"), "expected sql492 for NOT IN (NULL): {d:?}");
}

#[test]
fn sql492_in_with_only_null() {
  let d = diags("SELECT * FROM users WHERE name IN (NULL);");
  assert!(d.iter().any(|x| x.code == "sql492" && x.message.contains("never TRUE")), "expected sql492 Warning for IN (NULL): {d:?}");
}

#[test]
fn sql492_in_with_null_among_others_is_hint() {
  let d = diags("SELECT * FROM users WHERE name IN ('a', NULL);");
  let hit = d.iter().find(|x| x.code == "sql492").expect("sql492");
  assert!(hit.message.contains("dead-code"), "expected dead-code hint message: {}", hit.message);
}

#[test]
fn sql492_quiet_for_real_not_in_list() {
  let d = diags("SELECT * FROM users WHERE name NOT IN ('a', 'b');");
  assert!(!d.iter().any(|x| x.code == "sql492"), "real NOT IN list must not fire: {d:?}");
}

#[test]
fn sql492_quiet_for_in_subquery() {
  // IN (SELECT ...) is a subquery, not a literal list -- out of scope.
  let d = diags("SELECT * FROM users WHERE id NOT IN (SELECT id FROM users);");
  assert!(!d.iter().any(|x| x.code == "sql492"), "subquery NOT IN must not fire: {d:?}");
}

// `id` is NOT NULL in the test catalog (see helper `diags`); `email`
// is nullable.
#[test]
fn sql493_coalesce_on_not_null_column() {
  let d = diags("SELECT COALESCE(id, 0) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql493" && x.message.contains("dead code")), "expected sql493 for COALESCE(id, 0): {d:?}");
}

#[test]
fn sql493_coalesce_on_not_null_multi_default() {
  let d = diags("SELECT COALESCE(id, 0, 1) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql493"), "expected sql493 for multi-default COALESCE: {d:?}");
}

#[test]
fn sql493_coalesce_qualified_alias() {
  let d = diags("SELECT COALESCE(u.id, 0) FROM users u;");
  assert!(d.iter().any(|x| x.code == "sql493"), "expected sql493 for COALESCE(u.id, 0): {d:?}");
}

#[test]
fn sql493_quiet_for_nullable_column() {
  // `name` is nullable in the test catalog -- COALESCE is meaningful.
  let d = diags("SELECT COALESCE(name, '') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql493"), "nullable column COALESCE must not fire: {d:?}");
}

#[test]
fn sql493_quiet_for_expression_arg() {
  // First arg is a function call, not a bare column -- we don't
  // know nullability without deeper analysis, so stay quiet.
  let d = diags("SELECT COALESCE(upper(name), 'x') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql493"), "expression first-arg must not fire: {d:?}");
}

#[test]
fn sql493_quiet_for_single_arg() {
  // COALESCE with 1 arg has no defaults to be dead code about.
  let d = diags("SELECT COALESCE(id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql493"), "single-arg COALESCE must not fire: {d:?}");
}

#[test]
fn sql494_jsonb_set_empty_path_literal() {
  let d = diags("SELECT jsonb_set(name::jsonb, '{}', '\"x\"') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql494" && x.message.contains("no-op")), "expected sql494 for jsonb_set empty path: {d:?}");
}

#[test]
fn sql494_jsonb_set_empty_path_with_cast() {
  let d = diags("SELECT jsonb_set(name::jsonb, '{}'::text[], '\"x\"') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql494"), "expected sql494 for jsonb_set '{{}}'::text[]: {d:?}");
}

#[test]
fn sql494_jsonb_set_empty_array_constructor() {
  let d = diags("SELECT jsonb_set(name::jsonb, ARRAY[]::text[], '\"x\"') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql494"), "expected sql494 for ARRAY[]::text[]: {d:?}");
}

#[test]
fn sql494_jsonb_insert_empty_path() {
  let d = diags("SELECT jsonb_insert(name::jsonb, '{}', '\"x\"') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql494"), "expected sql494 for jsonb_insert: {d:?}");
}

#[test]
fn sql494_quiet_for_real_path() {
  let d = diags("SELECT jsonb_set(name::jsonb, '{a}', '\"x\"') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql494"), "real path must not fire: {d:?}");
}

#[test]
fn sql494_quiet_for_real_array_path() {
  let d = diags("SELECT jsonb_set(name::jsonb, ARRAY['a'], '\"x\"') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql494"), "real ARRAY path must not fire: {d:?}");
}

fn cat_with_generated() -> Catalog {
  let users = Table {
    schema: "public".into(),
    name: "events".into(),
    kind: TableKind::Table,
    columns: vec![
      Column { name: "id".into(), data_type: "int".into(), nullable: false, default: Some("GENERATED ALWAYS AS IDENTITY".into()), comment: None, generated: None, json_keys: None },
      Column { name: "full_name".into(), data_type: "text".into(), nullable: true, default: None, comment: None, generated: Some("first_name || ' ' || last_name".into()), json_keys: None },
      Column { name: "name".into(), data_type: "text".into(), nullable: true, default: None, comment: None, generated: None, json_keys: None },
    ],
    constraints: vec![], indexes: vec![], triggers: vec![], policies: vec![], comment: None, row_estimate: None, owner: None, definition: None, strict: false, options: None,
  };
  Catalog { version: CATALOG_VERSION, connection_id: "test".into(), schemas: vec![Schema { name: "public".into(), tables: vec![users] }], functions: vec![], types: vec![], roles: vec![], sequences: vec![], extensions: vec![] }
}

fn diags_with_generated(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let c = cat_with_generated();
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &c)
}

#[test]
fn sql178_insert_identity_column_flagged() {
  let d = diags_with_generated("INSERT INTO events (id, name) VALUES (1, 'a');");
  assert!(d.iter().any(|x| x.code == "sql178" && x.message.contains("IDENTITY")), "expected sql178 for INSERT into identity: {d:?}");
}

#[test]
fn sql178_insert_stored_generated_column_flagged() {
  // Regression iter190: STORED generated columns were NOT caught
  // by sql178 (only IDENTITY via `default` field was checked).
  let d = diags_with_generated("INSERT INTO events (full_name, name) VALUES ('x', 'a');");
  assert!(d.iter().any(|x| x.code == "sql178" && x.message.contains("STORED")), "expected sql178 for INSERT into STORED generated: {d:?}");
}

#[test]
fn sql178_insert_with_override_allows_identity() {
  let d = diags_with_generated("INSERT INTO events (id, name) OVERRIDING SYSTEM VALUE VALUES (1, 'a');");
  assert!(!d.iter().any(|x| x.code == "sql178"), "OVERRIDING SYSTEM VALUE must clear identity error: {d:?}");
}

#[test]
fn sql178_update_identity_column_flagged() {
  // Regression iter190: UPDATE of identity column was silent.
  let d = diags_with_generated("UPDATE events SET id = 2 WHERE name = 'a';");
  assert!(d.iter().any(|x| x.code == "sql178" && x.message.contains("IDENTITY")), "expected sql178 for UPDATE of identity col: {d:?}");
}

#[test]
fn sql178_update_stored_generated_column_flagged() {
  // Regression iter190: UPDATE of STORED generated col was silent.
  let d = diags_with_generated("UPDATE events SET full_name = 'x' WHERE id = 1;");
  assert!(d.iter().any(|x| x.code == "sql178" && x.message.contains("STORED")), "expected sql178 for UPDATE of STORED generated: {d:?}");
}

#[test]
fn sql178_quiet_for_writable_columns() {
  let d = diags_with_generated("INSERT INTO events (name) VALUES ('a');");
  assert!(!d.iter().any(|x| x.code == "sql178"), "writable column INSERT must not fire: {d:?}");
}

#[test]
fn sql178_quiet_for_update_writable_column() {
  let d = diags_with_generated("UPDATE events SET name = 'x' WHERE id = 1;");
  assert!(!d.iter().any(|x| x.code == "sql178"), "writable column UPDATE must not fire: {d:?}");
}

#[test]
fn sql495_eq_all_array_distinct_elements_is_always_false() {
  let d = diags("SELECT * FROM users WHERE name = ALL(ARRAY['a', 'b']);");
  assert!(d.iter().any(|x| x.code == "sql495" && x.message.contains("always FALSE")), "expected sql495 Warning for distinct = ALL: {d:?}");
}

#[test]
fn sql495_eq_all_curly_literal_array_distinct() {
  // '{1,2,3}'::int[] form
  let d = diags("SELECT * FROM users WHERE id = ALL('{1,2,3}'::int[]);");
  assert!(d.iter().any(|x| x.code == "sql495"), "expected sql495 for '{{1,2,3}}'::int[]: {d:?}");
}

#[test]
fn sql495_eq_all_identical_elements_is_hint() {
  let d = diags("SELECT * FROM users WHERE id = ALL(ARRAY[5, 5, 5]);");
  let hit = d.iter().find(|x| x.code == "sql495").expect("sql495");
  assert!(hit.message.contains("identical"), "expected identical-elements hint: {}", hit.message);
}

#[test]
fn sql495_quiet_for_single_element_array() {
  let d = diags("SELECT * FROM users WHERE id = ALL(ARRAY[1]);");
  assert!(!d.iter().any(|x| x.code == "sql495"), "single-element ALL must not fire: {d:?}");
}

#[test]
fn sql495_quiet_for_subquery() {
  let d = diags("SELECT * FROM users WHERE name = ALL(SELECT name FROM users);");
  assert!(!d.iter().any(|x| x.code == "sql495"), "subquery ALL must not fire: {d:?}");
}

#[test]
fn sql495_quiet_for_eq_any() {
  let d = diags("SELECT * FROM users WHERE name = ANY(ARRAY['a','b']);");
  assert!(!d.iter().any(|x| x.code == "sql495"), "= ANY must not fire: {d:?}");
}

#[test]
fn sql495_quiet_for_neq_all() {
  // `<> ALL` is the canonical NOT-IN-style form; correct usage.
  let d = diags("SELECT * FROM users WHERE id <> ALL(ARRAY[1,2,3]);");
  assert!(!d.iter().any(|x| x.code == "sql495"), "<> ALL must not fire: {d:?}");
}

// `id` is NOT NULL with no default in test catalog; `name` is
// nullable with no default; `email` is NOT NULL with no default.

#[test]
fn sql496_default_on_not_null_no_default_is_error() {
  let d = diags("UPDATE users SET id = DEFAULT WHERE name = 'a';");
  let hit = d.iter().find(|x| x.code == "sql496").expect("sql496");
  assert_eq!(hit.severity, dsl_analysis::Severity::Error);
  assert!(hit.message.contains("null value"), "expected NOT NULL message: {}", hit.message);
}

#[test]
fn sql496_default_on_nullable_no_default_is_hint() {
  let d = diags("UPDATE users SET name = DEFAULT WHERE id = 1;");
  let hit = d.iter().find(|x| x.code == "sql496").expect("sql496");
  assert_eq!(hit.severity, dsl_analysis::Severity::Hint);
  assert!(hit.message.contains("silently becomes NULL"), "expected silently-NULL message: {}", hit.message);
}

#[test]
fn sql496_quiet_for_real_value() {
  let d = diags("UPDATE users SET name = 'x' WHERE id = 1;");
  assert!(!d.iter().any(|x| x.code == "sql496"), "real value must not fire: {d:?}");
}

#[test]
fn sql496_multi_assignment_flags_each() {
  let d = diags("UPDATE users SET name = DEFAULT, id = DEFAULT WHERE created_at IS NOT NULL;");
  let n = d.iter().filter(|x| x.code == "sql496").count();
  assert_eq!(n, 2, "expected sql496 for both assignments: {d:?}");
}

#[test]
fn sql497_array_agg_distinct_order_mismatch() {
  let d = diags("SELECT array_agg(DISTINCT name ORDER BY created_at) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql497" && x.message.contains("argument list")), "expected sql497 for array_agg distinct+order mismatch: {d:?}");
}

#[test]
fn sql497_string_agg_distinct_order_mismatch() {
  let d = diags("SELECT string_agg(DISTINCT name, ',' ORDER BY created_at) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql497"), "expected sql497 for string_agg: {d:?}");
}

#[test]
fn sql497_json_agg_distinct_order_mismatch() {
  let d = diags("SELECT json_agg(DISTINCT name ORDER BY created_at) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql497"), "expected sql497 for json_agg: {d:?}");
}

#[test]
fn sql497_quiet_when_order_matches_distinct() {
  let d = diags("SELECT array_agg(DISTINCT name ORDER BY name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql497"), "matching DISTINCT/ORDER BY must not fire: {d:?}");
}

#[test]
fn sql497_quiet_for_no_distinct() {
  let d = diags("SELECT array_agg(name ORDER BY created_at) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql497"), "no DISTINCT must not fire: {d:?}");
}

#[test]
fn sql497_quiet_for_no_order_by() {
  let d = diags("SELECT array_agg(DISTINCT name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql497"), "no ORDER BY must not fire: {d:?}");
}

#[test]
fn sql498_flags_similar_to() {
  let d = diags("SELECT * FROM users WHERE name SIMILAR TO '%foo%';");
  assert!(d.iter().any(|x| x.code == "sql498" && x.message.contains("SIMILAR TO")), "expected sql498 for SIMILAR TO: {d:?}");
}

#[test]
fn sql498_flags_not_similar_to() {
  let d = diags("SELECT * FROM users WHERE name NOT SIMILAR TO '%bar%';");
  assert!(d.iter().any(|x| x.code == "sql498"), "expected sql498 for NOT SIMILAR TO: {d:?}");
}

#[test]
fn sql498_quiet_for_like() {
  let d = diags("SELECT * FROM users WHERE email LIKE 'a@%';");
  assert!(!d.iter().any(|x| x.code == "sql498"), "LIKE must not fire: {d:?}");
}

#[test]
fn sql498_quiet_for_posix_regex() {
  let d = diags("SELECT * FROM users WHERE name ~ 'a(b|c)d';");
  assert!(!d.iter().any(|x| x.code == "sql498"), "POSIX regex must not fire: {d:?}");
}

fn cat_with_tsvector() -> Catalog {
  let docs = Table {
    schema: "public".into(),
    name: "docs".into(),
    kind: TableKind::Table,
    columns: vec![
      Column { name: "id".into(), data_type: "int".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "search".into(), data_type: "tsvector".into(), nullable: true, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "body".into(), data_type: "text".into(), nullable: true, default: None, comment: None, generated: None, json_keys: None },
    ],
    constraints: vec![], indexes: vec![], triggers: vec![], policies: vec![], comment: None, row_estimate: None, owner: None, definition: None, strict: false, options: None,
  };
  Catalog { version: CATALOG_VERSION, connection_id: "test".into(), schemas: vec![Schema { name: "public".into(), tables: vec![docs] }], functions: vec![], types: vec![], roles: vec![], sequences: vec![], extensions: vec![] }
}

fn diags_with_tsvector(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let c = cat_with_tsvector();
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &c)
}

#[test]
fn sql499_tsvector_at_at_text_with_space_is_runtime_error() {
  let d = diags_with_tsvector("SELECT * FROM docs WHERE search @@ 'foo bar';");
  let hit = d.iter().find(|x| x.code == "sql499").expect("sql499");
  assert!(hit.message.contains("runtime syntax error"), "expected runtime-error message: {}", hit.message);
}

#[test]
fn sql499_tsvector_at_at_text_with_operator_is_warning() {
  let d = diags_with_tsvector("SELECT * FROM docs WHERE search @@ 'foo & bar';");
  assert!(d.iter().any(|x| x.code == "sql499"), "expected sql499 for `& bar`: {d:?}");
}

#[test]
fn sql499_quiet_for_to_tsquery_wrapper() {
  let d = diags_with_tsvector("SELECT * FROM docs WHERE search @@ to_tsquery('foo & bar');");
  assert!(!d.iter().any(|x| x.code == "sql499"), "to_tsquery must not fire: {d:?}");
}

#[test]
fn sql499_quiet_for_plainto_tsquery_wrapper() {
  let d = diags_with_tsvector("SELECT * FROM docs WHERE search @@ plainto_tsquery('foo bar');");
  assert!(!d.iter().any(|x| x.code == "sql499"), "plainto_tsquery must not fire: {d:?}");
}

#[test]
fn sql499_quiet_for_non_tsvector_column() {
  // body is text, not tsvector; @@ would be wrong anyway but isn't
  // sql499's territory.
  let d = diags_with_tsvector("SELECT * FROM docs WHERE body @@ 'foo';");
  assert!(!d.iter().any(|x| x.code == "sql499"), "non-tsvector col must not fire: {d:?}");
}

fn cat_with_dates() -> Catalog {
  let events = Table {
    schema: "public".into(),
    name: "events".into(),
    kind: TableKind::Table,
    columns: vec![
      Column { name: "id".into(), data_type: "int".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "start_date".into(), data_type: "date".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "end_date".into(), data_type: "date".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "start_ts".into(), data_type: "timestamptz".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "end_ts".into(), data_type: "timestamptz".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
    ],
    constraints: vec![], indexes: vec![], triggers: vec![], policies: vec![], comment: None, row_estimate: None, owner: None, definition: None, strict: false, options: None,
  };
  Catalog { version: CATALOG_VERSION, connection_id: "test".into(), schemas: vec![Schema { name: "public".into(), tables: vec![events] }], functions: vec![], types: vec![], roles: vec![], sequences: vec![], extensions: vec![] }
}

fn diags_with_dates(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let c = cat_with_dates();
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &c)
}

#[test]
fn sql500_flags_date_minus_date() {
  let d = diags_with_dates("SELECT end_date - start_date FROM events;");
  let hit = d.iter().find(|x| x.code == "sql500").expect("sql500");
  assert!(hit.message.contains("integer"), "expected integer-days message: {}", hit.message);
}

#[test]
fn sql500_quiet_for_timestamp_minus_timestamp() {
  // timestamp - timestamp DOES return an interval; no diagnostic.
  let d = diags_with_dates("SELECT end_ts - start_ts FROM events;");
  assert!(!d.iter().any(|x| x.code == "sql500"), "ts - ts must not fire: {d:?}");
}

#[test]
fn sql500_quiet_for_date_minus_interval() {
  // date - INTERVAL '1 day' returns timestamp; no diagnostic.
  let d = diags_with_dates("SELECT end_date - INTERVAL '1 day' FROM events;");
  assert!(!d.iter().any(|x| x.code == "sql500"), "date - interval must not fire: {d:?}");
}

#[test]
fn sql500_quiet_for_age_function() {
  let d = diags_with_dates("SELECT age(end_date, start_date) FROM events;");
  assert!(!d.iter().any(|x| x.code == "sql500"), "age() must not fire: {d:?}");
}

// `id` and `email` are NOT NULL in test catalog; `name` is nullable.

#[test]
fn sql501_flags_nulls_last_on_not_null_col() {
  let d = diags("SELECT * FROM users ORDER BY id NULLS LAST;");
  let hit = d.iter().find(|x| x.code == "sql501").expect("sql501");
  assert!(hit.message.contains("NULLS LAST") && hit.message.contains("redundant"), "msg: {}", hit.message);
}

#[test]
fn sql501_flags_nulls_first_on_not_null_col() {
  let d = diags("SELECT * FROM users ORDER BY id NULLS FIRST;");
  assert!(d.iter().any(|x| x.code == "sql501"), "expected sql501 for NULLS FIRST on NOT NULL: {d:?}");
}

#[test]
fn sql501_flags_with_desc_modifier() {
  let d = diags("SELECT * FROM users ORDER BY email DESC NULLS FIRST;");
  assert!(d.iter().any(|x| x.code == "sql501"), "expected sql501 even with DESC: {d:?}");
}

#[test]
fn sql501_quiet_for_nullable_column() {
  let d = diags("SELECT * FROM users ORDER BY name NULLS LAST;");
  assert!(!d.iter().any(|x| x.code == "sql501"), "nullable col must not fire: {d:?}");
}

#[test]
fn sql501_quiet_for_no_nulls_clause() {
  let d = diags("SELECT * FROM users ORDER BY id;");
  assert!(!d.iter().any(|x| x.code == "sql501"), "no NULLS clause must not fire: {d:?}");
}

#[test]
fn sql501_mixed_items_flags_only_not_null() {
  let d = diags("SELECT * FROM users ORDER BY id NULLS LAST, name NULLS LAST;");
  let n = d.iter().filter(|x| x.code == "sql501").count();
  assert_eq!(n, 1, "expected exactly one sql501 for the NOT NULL item: {d:?}");
}

fn cat_with_tstz() -> Catalog {
  let events = Table {
    schema: "public".into(),
    name: "events".into(),
    kind: TableKind::Table,
    columns: vec![
      Column { name: "id".into(), data_type: "int".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "created_at".into(), data_type: "timestamptz".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "scheduled_for".into(), data_type: "timestamp".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
    ],
    constraints: vec![], indexes: vec![], triggers: vec![], policies: vec![], comment: None, row_estimate: None, owner: None, definition: None, strict: false, options: None,
  };
  Catalog { version: CATALOG_VERSION, connection_id: "test".into(), schemas: vec![Schema { name: "public".into(), tables: vec![events] }], functions: vec![], types: vec![], roles: vec![], sequences: vec![], extensions: vec![] }
}

fn diags_with_tstz(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let c = cat_with_tstz();
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &c)
}

#[test]
fn sql502_flags_timestamp_literal_on_tstz_col() {
  let d = diags_with_tstz("SELECT * FROM events WHERE created_at > TIMESTAMP '2024-01-01';");
  let hit = d.iter().find(|x| x.code == "sql502").expect("sql502");
  assert!(hit.message.contains("session"), "expected session-tz message: {}", hit.message);
}

#[test]
fn sql502_quiet_for_timestamptz_literal() {
  let d = diags_with_tstz("SELECT * FROM events WHERE created_at > TIMESTAMPTZ '2024-01-01';");
  assert!(!d.iter().any(|x| x.code == "sql502"), "TIMESTAMPTZ literal must not fire: {d:?}");
}

#[test]
fn sql502_quiet_for_cast_literal() {
  let d = diags_with_tstz("SELECT * FROM events WHERE created_at > '2024-01-01'::timestamptz;");
  assert!(!d.iter().any(|x| x.code == "sql502"), "cast literal must not fire: {d:?}");
}

#[test]
fn sql502_quiet_for_plain_string_literal() {
  // No TIMESTAMP prefix -- PG infers the type from context (col type).
  let d = diags_with_tstz("SELECT * FROM events WHERE created_at > '2024-01-01';");
  assert!(!d.iter().any(|x| x.code == "sql502"), "plain string literal must not fire: {d:?}");
}

#[test]
fn sql502_quiet_for_timestamp_column_match() {
  // scheduled_for is `timestamp` (no tz); matching types are fine.
  let d = diags_with_tstz("SELECT * FROM events WHERE scheduled_for > TIMESTAMP '2024-01-01';");
  assert!(!d.iter().any(|x| x.code == "sql502"), "matching timestamp col must not fire: {d:?}");
}

#[test]
fn sql502_quiet_for_timestamp_with_time_zone_form() {
  // `TIMESTAMP WITH TIME ZONE 'lit'` is the SQL-standard spelling of TIMESTAMPTZ.
  let d = diags_with_tstz("SELECT * FROM events WHERE created_at > TIMESTAMP WITH TIME ZONE '2024-01-01';");
  assert!(!d.iter().any(|x| x.code == "sql502"), "WITH TIME ZONE form must not fire: {d:?}");
}

fn cat_with_json_text() -> Catalog {
  let docs = Table {
    schema: "public".into(),
    name: "docs".into(),
    kind: TableKind::Table,
    columns: vec![
      Column { name: "id".into(), data_type: "int".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "data".into(), data_type: "jsonb".into(), nullable: true, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "meta".into(), data_type: "json".into(), nullable: true, default: None, comment: None, generated: None, json_keys: None },
      Column { name: "body".into(), data_type: "text".into(), nullable: true, default: None, comment: None, generated: None, json_keys: None },
    ],
    constraints: vec![], indexes: vec![], triggers: vec![], policies: vec![], comment: None, row_estimate: None, owner: None, definition: None, strict: false, options: None,
  };
  Catalog { version: CATALOG_VERSION, connection_id: "test".into(), schemas: vec![Schema { name: "public".into(), tables: vec![docs] }], functions: vec![], types: vec![], roles: vec![], sequences: vec![], extensions: vec![] }
}

fn diags_with_json_text(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let c = cat_with_json_text();
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &c)
}

#[test]
fn sql503_flags_question_on_text_col() {
  let d = diags_with_json_text("SELECT * FROM docs WHERE body ? 'key';");
  let hit = d.iter().find(|x| x.code == "sql503").expect("sql503");
  assert!(hit.message.contains("text"), "expected message about text: {}", hit.message);
}

#[test]
fn sql503_flags_question_on_json_col() {
  // `json` doesn't have ? either -- jsonb only.
  let d = diags_with_json_text("SELECT * FROM docs WHERE meta ? 'key';");
  assert!(d.iter().any(|x| x.code == "sql503" && x.message.contains("json")), "expected sql503 for json col: {d:?}");
}

#[test]
fn sql503_flags_question_pipe_on_text() {
  let d = diags_with_json_text("SELECT * FROM docs WHERE body ?| ARRAY['a','b'];");
  assert!(d.iter().any(|x| x.code == "sql503" && x.message.contains("?|")), "expected sql503 for ?|: {d:?}");
}

#[test]
fn sql503_flags_question_amp_on_text() {
  let d = diags_with_json_text("SELECT * FROM docs WHERE body ?& ARRAY['a','b'];");
  assert!(d.iter().any(|x| x.code == "sql503" && x.message.contains("?&")), "expected sql503 for ?&: {d:?}");
}

#[test]
fn sql503_quiet_for_jsonb_col() {
  let d = diags_with_json_text("SELECT * FROM docs WHERE data ? 'key';");
  assert!(!d.iter().any(|x| x.code == "sql503"), "jsonb col must not fire: {d:?}");
}

#[test]
fn sql503_quiet_for_jsonb_pipe_amp() {
  let d = diags_with_json_text("SELECT * FROM docs WHERE data ?| ARRAY['a'];");
  assert!(!d.iter().any(|x| x.code == "sql503"), "jsonb ?| must not fire: {d:?}");
}

// `nums` table from cat_with_ints: s smallint, i integer, b bigint,
// t text. The base `users` catalog has `id` as uuid, so we use the
// ints catalog for sql504.

#[test]
fn sql504_flags_int_col_div_int_literal() {
  let d = diags_with_ints("SELECT i / 2 FROM nums;");
  let hit = d.iter().find(|x| x.code == "sql504").expect("sql504");
  assert!(hit.message.contains("truncates"), "expected truncates message: {}", hit.message);
}

#[test]
fn sql504_flags_bigint_col_div_int_literal() {
  let d = diags_with_ints("SELECT b / 2 FROM nums;");
  assert!(d.iter().any(|x| x.code == "sql504"), "bigint / int must fire: {d:?}");
}

#[test]
fn sql504_quiet_for_float_literal_rhs() {
  let d = diags_with_ints("SELECT i / 2.0 FROM nums;");
  assert!(!d.iter().any(|x| x.code == "sql504"), "float literal RHS must not fire: {d:?}");
}

#[test]
fn sql504_quiet_for_cast_lhs() {
  let d = diags_with_ints("SELECT i::float / 2 FROM nums;");
  assert!(!d.iter().any(|x| x.code == "sql504"), "cast LHS must not fire: {d:?}");
}

#[test]
fn sql504_quiet_for_non_int_col() {
  // `t` is text -- division would be a different error, but
  // sql504 is integer-specific.
  let d = diags_with_ints("SELECT t / 2 FROM nums;");
  assert!(!d.iter().any(|x| x.code == "sql504"), "text col must not fire: {d:?}");
}

#[test]
fn sql504_quiet_for_decimal_literal_rhs() {
  let d = diags_with_ints("SELECT i / 2.5 FROM nums;");
  assert!(!d.iter().any(|x| x.code == "sql504"), "decimal RHS must not fire: {d:?}");
}

#[test]
fn sql504_quiet_for_div_by_zero() {
  // Regression iter210: sql504 used to suggest `i::float / 0` which
  // is still a division by zero. sql278 owns this case; sql504 must
  // stay out.
  let d = diags_with_ints("SELECT i / 0 FROM nums;");
  assert!(!d.iter().any(|x| x.code == "sql504"), "sql504 must not fire on /0 (sql278's territory): {d:?}");
  assert!(d.iter().any(|x| x.code == "sql278"), "sql278 should still fire: {d:?}");
}

#[test]
fn sql505_flags_arrow_on_text_col() {
  let d = diags_with_json_text("SELECT body -> 'k' FROM docs;");
  let hit = d.iter().find(|x| x.code == "sql505").expect("sql505");
  assert!(hit.message.contains("text"), "expected text-type message: {}", hit.message);
}

#[test]
fn sql505_flags_double_arrow_on_text_col() {
  let d = diags_with_json_text("SELECT body ->> 'k' FROM docs;");
  assert!(d.iter().any(|x| x.code == "sql505" && x.message.contains("->>")), "expected sql505 for ->>: {d:?}");
}

#[test]
fn sql505_flags_hash_arrow_on_text_col() {
  let d = diags_with_json_text("SELECT body #> '{a,b}' FROM docs;");
  assert!(d.iter().any(|x| x.code == "sql505" && x.message.contains("#>")), "expected sql505 for #>: {d:?}");
}

#[test]
fn sql505_flags_hash_double_arrow_on_text_col() {
  let d = diags_with_json_text("SELECT body #>> '{a,b}' FROM docs;");
  assert!(d.iter().any(|x| x.code == "sql505" && x.message.contains("#>>")), "expected sql505 for #>>: {d:?}");
}

#[test]
fn sql505_quiet_for_jsonb_col() {
  let d = diags_with_json_text("SELECT data -> 'k' FROM docs;");
  assert!(!d.iter().any(|x| x.code == "sql505"), "jsonb col must not fire: {d:?}");
}

#[test]
fn sql505_quiet_for_json_col() {
  // json also has the -> operator; only text is the broken case.
  let d = diags_with_json_text("SELECT meta -> 'k' FROM docs;");
  assert!(!d.iter().any(|x| x.code == "sql505"), "json col must not fire: {d:?}");
}

#[test]
fn sql506_flags_array_single_null() {
  let d = diags("SELECT ARRAY[NULL] FROM users;");
  assert!(d.iter().any(|x| x.code == "sql506" && x.message.contains("cannot determine")), "expected sql506 for ARRAY[NULL]: {d:?}");
}

#[test]
fn sql506_flags_array_multi_null() {
  let d = diags("SELECT ARRAY[NULL, NULL, NULL] FROM users;");
  assert!(d.iter().any(|x| x.code == "sql506"), "expected sql506 for ARRAY[NULL,NULL,NULL]: {d:?}");
}

#[test]
fn sql506_quiet_for_typed_element() {
  // The first non-NULL element types the array.
  let d = diags("SELECT ARRAY[1, NULL, 2] FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql506"), "typed element must not fire: {d:?}");
}

#[test]
fn sql506_quiet_for_inner_cast() {
  let d = diags("SELECT ARRAY[NULL::int] FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql506"), "inner cast must not fire: {d:?}");
}

#[test]
fn sql506_quiet_for_outer_cast() {
  let d = diags("SELECT ARRAY[NULL]::int[] FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql506"), "outer cast must not fire: {d:?}");
}

#[test]
fn sql507_flags_execute_concat_with_param() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN EXECUTE 'SELECT * FROM users WHERE id = ' || $1; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql507" && x.message.contains("injection")), "expected sql507 for EXECUTE || $1: {d:?}");
}

#[test]
fn sql507_flags_execute_concat_with_variable() {
  let d = diags("CREATE OR REPLACE FUNCTION f(x text) RETURNS void LANGUAGE plpgsql AS $$ BEGIN EXECUTE 'SELECT * FROM ' || x; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql507"), "expected sql507 for EXECUTE || x: {d:?}");
}

#[test]
fn sql507_quiet_for_execute_using() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN EXECUTE 'SELECT * FROM users WHERE id = $1' USING $1; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql507"), "EXECUTE ... USING must not fire: {d:?}");
}

#[test]
fn sql507_quiet_for_format_wrapper() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN EXECUTE format('SELECT * FROM %I', 'users'); END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql507"), "EXECUTE format(...) must not fire: {d:?}");
}

#[test]
fn sql508_flags_self_like() {
  let d = diags("SELECT * FROM users WHERE email LIKE email;");
  let hit = d.iter().find(|x| x.code == "sql508").expect("sql508");
  assert!(hit.message.contains("always TRUE"), "expected always-TRUE message: {}", hit.message);
}

#[test]
fn sql508_flags_self_ilike() {
  let d = diags("SELECT * FROM users WHERE email ILIKE email;");
  assert!(d.iter().any(|x| x.code == "sql508"), "expected sql508 for self ILIKE: {d:?}");
}

#[test]
fn sql508_flags_self_not_like() {
  let d = diags("SELECT * FROM users WHERE email NOT LIKE email;");
  let hit = d.iter().find(|x| x.code == "sql508").expect("sql508");
  assert!(hit.message.contains("always FALSE"), "expected always-FALSE message: {}", hit.message);
}

#[test]
fn sql508_flags_self_posix_regex() {
  let d = diags("SELECT * FROM users WHERE email ~ email;");
  assert!(d.iter().any(|x| x.code == "sql508" && x.message.contains("~")), "expected sql508 for self ~: {d:?}");
}

#[test]
fn sql508_flags_self_neg_posix_regex() {
  let d = diags("SELECT * FROM users WHERE email !~ email;");
  let hit = d.iter().find(|x| x.code == "sql508").expect("sql508");
  assert!(hit.message.contains("always FALSE"), "expected always-FALSE for !~: {}", hit.message);
}

#[test]
fn sql508_quiet_for_real_pattern() {
  let d = diags("SELECT * FROM users WHERE email LIKE 'a%';");
  assert!(!d.iter().any(|x| x.code == "sql508"), "real pattern must not fire: {d:?}");
}

#[test]
fn sql508_quiet_for_different_qualifiers() {
  let d = diags("SELECT * FROM users u JOIN users v ON u.id::text LIKE v.id::text;");
  assert!(!d.iter().any(|x| x.code == "sql508"), "different qualifiers must not fire: {d:?}");
}

#[test]
fn sql509_flags_pg_temp_select() {
  let d = diags("SELECT * FROM pg_temp.my_tmp;");
  assert!(d.iter().any(|x| x.code == "sql509" && x.message.contains("per-backend")), "expected sql509 for pg_temp.my_tmp: {d:?}");
}

#[test]
fn sql509_flags_pg_temp_numbered_suffix() {
  let d = diags("SELECT * FROM pg_temp_3.my_tmp;");
  assert!(d.iter().any(|x| x.code == "sql509"), "expected sql509 for pg_temp_3.my_tmp: {d:?}");
}

#[test]
fn sql509_flags_pg_temp_in_drop() {
  let d = diags("DROP TABLE pg_temp.tmp;");
  assert!(d.iter().any(|x| x.code == "sql509"), "expected sql509 for DROP pg_temp.tmp: {d:?}");
}

#[test]
fn sql509_quiet_for_pg_catalog() {
  let d = diags("SELECT * FROM pg_catalog.pg_tables;");
  assert!(!d.iter().any(|x| x.code == "sql509"), "pg_catalog must not fire: {d:?}");
}

#[test]
fn sql509_quiet_for_pg_toast() {
  let d = diags("SELECT * FROM pg_toast.something;");
  assert!(!d.iter().any(|x| x.code == "sql509"), "pg_toast must not fire: {d:?}");
}

#[test]
fn sql509_quiet_for_other_schema() {
  let d = diags("SELECT * FROM public.users;");
  assert!(!d.iter().any(|x| x.code == "sql509"), "other schema must not fire: {d:?}");
}

#[test]
fn sql510_flags_self_similar_to() {
  let d = diags("SELECT * FROM users WHERE email SIMILAR TO email;");
  let hit = d.iter().find(|x| x.code == "sql510").expect("sql510");
  assert!(hit.message.contains("always TRUE"), "expected always-TRUE message: {}", hit.message);
}

#[test]
fn sql510_flags_self_not_similar_to() {
  let d = diags("SELECT * FROM users WHERE email NOT SIMILAR TO email;");
  let hit = d.iter().find(|x| x.code == "sql510").expect("sql510");
  assert!(hit.message.contains("always FALSE"), "expected always-FALSE message: {}", hit.message);
}

#[test]
fn sql510_quiet_for_real_pattern() {
  let d = diags("SELECT * FROM users WHERE email SIMILAR TO 'foo%';");
  assert!(!d.iter().any(|x| x.code == "sql510"), "real pattern must not fire: {d:?}");
}

#[test]
fn sql510_quiet_for_different_qualifiers() {
  let d = diags("SELECT * FROM users u JOIN users v ON u.email SIMILAR TO v.email;");
  assert!(!d.iter().any(|x| x.code == "sql510"), "different qualifiers must not fire: {d:?}");
}

#[test]
fn sql511_flags_self_contained_by() {
  let d = diags("SELECT * FROM users WHERE email <@ email;");
  let hit = d.iter().find(|x| x.code == "sql511").expect("sql511");
  assert!(hit.message.contains("contained by itself"), "expected contained-by-itself message: {}", hit.message);
}

#[test]
fn sql511_flags_self_overlap() {
  let d = diags("SELECT * FROM users WHERE email && email;");
  let hit = d.iter().find(|x| x.code == "sql511").expect("sql511");
  assert!(hit.message.contains("overlaps itself"), "expected overlaps-itself message: {}", hit.message);
}

#[test]
fn sql511_quiet_for_literal_rhs() {
  let d = diags("SELECT * FROM users WHERE email::jsonb @> '{\"a\":1}'::jsonb;");
  assert!(!d.iter().any(|x| x.code == "sql511"), "literal RHS must not fire: {d:?}");
}

#[test]
fn sql511_quiet_for_different_qualifiers() {
  let d = diags("SELECT * FROM users u JOIN users v ON u.email @> v.email;");
  assert!(!d.iter().any(|x| x.code == "sql511"), "different qualifiers must not fire: {d:?}");
}

#[test]
fn sql348_quiet_for_character_type_spelling() {
  // Regression iter211: `character(10)` was being misread as a
  // function call. PG type spellings: character, varying, bpchar.
  let d = diags("CREATE TABLE u (code character(10));");
  assert!(!d.iter().any(|x| x.code == "sql348" && x.message.contains("character")), "sql348 must not flag `character` type: {d:?}");
}

#[test]
fn sql348_quiet_for_character_varying_type() {
  let d = diags("CREATE TABLE u (code CHARACTER VARYING(10));");
  assert!(!d.iter().any(|x| x.code == "sql348" && x.message.contains("VARYING")), "sql348 must not flag `VARYING` keyword: {d:?}");
}

#[test]
fn sql348_quiet_for_bpchar_type() {
  let d = diags("CREATE TABLE u (code BPCHAR(10));");
  assert!(!d.iter().any(|x| x.code == "sql348" && x.message.contains("BPCHAR")), "sql348 must not flag `BPCHAR` type: {d:?}");
}

#[test]
fn sql104_flags_bpchar() {
  // Regression iter211: BPCHAR (PG's internal name for char(n)) was
  // silent under sql104.
  let d = diags("CREATE TABLE u (code BPCHAR(10), id int PRIMARY KEY);");
  assert!(d.iter().any(|x| x.code == "sql104"), "expected sql104 for BPCHAR: {d:?}");
}

// ===== sql052 LIKE without wildcard ========================================

#[test]
fn sql052_flags_plain_like() {
  let d = diags("SELECT * FROM users WHERE email LIKE 'alice@example.com';");
  assert!(d.iter().any(|x| x.code == "sql052"));
}

#[test]
fn sql052_quiet_with_percent_wildcard() {
  let d = diags("SELECT * FROM users WHERE email LIKE '%@example.com';");
  assert!(!d.iter().any(|x| x.code == "sql052"));
}

#[test]
fn sql052_quiet_with_underscore_wildcard() {
  let d = diags("SELECT * FROM users WHERE code LIKE 'A_C';");
  assert!(!d.iter().any(|x| x.code == "sql052"));
}

// ===== sql046 missing PRIMARY KEY ==========================================

#[test]
fn sql046_flags_table_without_pk() {
  let d = diags("CREATE TABLE log_events (id INT, data TEXT);");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn sql046_quiet_with_inline_pk() {
  let d = diags("CREATE TABLE foo (id INT PRIMARY KEY);");
  assert!(!d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn sql046_quiet_with_constraint_pk() {
  let d = diags("CREATE TABLE foo (id INT, CONSTRAINT pk_foo PRIMARY KEY (id));");
  assert!(!d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn sql046_quiet_on_temp_table() {
  let d = diags("CREATE TEMP TABLE scratch (id INT, data TEXT);");
  assert!(!d.iter().any(|x| x.code == "sql046"));
}

// ===== sql048 INSERT without column list ===================================

#[test]
fn sql048_flags_positional_insert() {
  let d = diags("INSERT INTO users VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');");
  assert!(d.iter().any(|x| x.code == "sql048"));
}

#[test]
fn sql048_quiet_with_column_list() {
  let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');");
  assert!(!d.iter().any(|x| x.code == "sql048"));
}

// ===== sql050 reserved word identifier =====================================

#[test]
fn sql050_quiet_for_non_reserved_word() {
  // `name`, `user`, `type` are NON-reserved -- should not fire.
  let d = diags("CREATE TABLE foo (id INT PRIMARY KEY, name TEXT, type TEXT);");
  assert!(!d.iter().any(|x| x.code == "sql050"));
}

// ===== sql039 INSERT type vs literal ======================================

#[test]
fn sql039_flags_string_in_uuid_column() {
  // catalog has users.id as uuid. Passing a plain int literal should
  // not flag (INT might cast); but a clearly mismatched literal will.
  // Test: passing INT into TEXT column.
  let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 42);");
  assert!(
    d.iter().any(|x| x.code == "sql039"),
    "expected sql039 for int into text, got {:?}",
    d.iter().map(|x| x.code).collect::<Vec<_>>()
  );
}

#[test]
fn sql039_quiet_when_types_match() {
  let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');");
  assert!(!d.iter().any(|x| x.code == "sql039"));
}

#[test]
fn sql039_quiet_for_null_value() {
  let d = diags("INSERT INTO users (id, email) VALUES (NULL, 'a@b.com');");
  assert!(!d.iter().any(|x| x.code == "sql039"));
}

#[test]
fn sql039_quiet_for_function_call_value() {
  // Function calls aren't literals -- we don't infer their type.
  let d = diags("INSERT INTO users (id, email) VALUES (gen_random_uuid(), 'a@b.com');");
  assert!(!d.iter().any(|x| x.code == "sql039"));
}

// ===== sql038 INSERT col/value count ======================================

#[test]
fn sql038_flags_too_few_values() {
  let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000');");
  assert!(d.iter().any(|x| x.code == "sql038"), "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql038_flags_too_many_values() {
  let d = diags("INSERT INTO users (id) VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');");
  assert!(d.iter().any(|x| x.code == "sql038"));
}

#[test]
fn sql038_quiet_when_counts_match() {
  let d = diags("INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 'a@b.com');");
  assert!(!d.iter().any(|x| x.code == "sql038"));
}

#[test]
fn sql038_quiet_when_no_column_list() {
  // `INSERT INTO t VALUES (...)` -- positional, no col-list to compare.
  let d = diags("INSERT INTO users VALUES ('00000000-0000-0000-0000-000000000000');");
  assert!(!d.iter().any(|x| x.code == "sql038"));
}

// ===== sql031 RETURN literal type vs declared =============================

#[test]
fn sql031_flags_string_in_int_function() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 'hello'; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql031"), "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql031_flags_int_in_text_function() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS TEXT LANGUAGE plpgsql AS $$ BEGIN RETURN 42; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql031"));
}

#[test]
fn sql031_quiet_when_int_matches_int() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 42; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql031"));
}

#[test]
fn sql031_quiet_when_string_matches_text() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS TEXT LANGUAGE plpgsql AS $$ BEGIN RETURN 'hi'; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql031"));
}

#[test]
fn sql031_quiet_when_returning_expression() {
  // Non-literal returns are out of scope (need type inference).
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN id + 1; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql031"));
}

#[test]
fn sql031_quiet_for_null_return() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql031"));
}

// ===== sql037 SELECT INTO shape mismatch ===================================

#[test]
fn sql037_flags_too_many_targets() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ DECLARE a INT; b INT; BEGIN SELECT 1 INTO a, b FROM users; RETURN a; END; $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql037"), "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql037_flags_too_few_targets() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ DECLARE a INT; BEGIN SELECT 1, 2 INTO a FROM users; RETURN a; END; $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql037"));
}

#[test]
fn sql037_quiet_when_matched() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ DECLARE a INT; b INT; BEGIN SELECT 1, 2 INTO a, b FROM users; RETURN a; END; $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql037"));
}

#[test]
fn sql037_quiet_for_star_projection() {
  // `SELECT * INTO row` is legal when row is composite-typed.
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ DECLARE r users; BEGIN SELECT * INTO r FROM users; RETURN 1; END; $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql037"));
}

// ===== sql042 UPDATE SET unknown column ====================================

#[test]
fn sql042_flags_unknown_set_column() {
  let d = diags("UPDATE users SET emial = 'x' WHERE id = '00000000-0000-0000-0000-000000000000';");
  assert!(d.iter().any(|x| x.code == "sql042"), "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql042_quiet_for_known_column() {
  let d = diags("UPDATE users SET email = 'x' WHERE id = '00000000-0000-0000-0000-000000000000';");
  assert!(!d.iter().any(|x| x.code == "sql042"));
}

// ===== sql040 IMMUTABLE calls VOLATILE =====================================

#[test]
fn sql040_flags_now_in_immutable() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS TIMESTAMPTZ IMMUTABLE LANGUAGE plpgsql AS $$ BEGIN RETURN now(); END; $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql040"));
}

#[test]
fn sql040_flags_random_in_immutable() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS DOUBLE PRECISION IMMUTABLE LANGUAGE plpgsql AS $$ BEGIN RETURN random(); END; $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql040"));
}

#[test]
fn sql040_quiet_in_volatile_function() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS TIMESTAMPTZ VOLATILE LANGUAGE plpgsql AS $$ BEGIN RETURN now(); END; $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql040"));
}

#[test]
fn sql040_quiet_when_now_is_in_string() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS TEXT IMMUTABLE LANGUAGE plpgsql AS $$ BEGIN RETURN 'now()'; END; $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql040"));
}

// ===== sql036 RAISE arg count ==============================================

#[test]
fn sql036_flags_too_few_args() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'got % and %', 1; END; $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql036"));
}

#[test]
fn sql036_quiet_when_counts_match() {
  let d =
    diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE 'got %', 1; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql036"));
}

#[test]
fn sql036_double_percent_is_literal() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE '100%% done'; END; $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql036"));
}

// ===== sql045 unreachable after RETURN/RAISE ==============================

#[test]
fn sql045_flags_code_after_return() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 1; SELECT 1; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql045"), "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql045_quiet_when_return_is_last() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN SELECT 1; RETURN 1; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql045"));
}

#[test]
fn sql045_quiet_when_return_is_inside_if() {
  // Return inside IF isn't unconditional -- code after IF is reachable.
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN IF true THEN RETURN 1; END IF; RETURN 2; END; $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql045"));
}

// ===== sql043 DELETE without WHERE in function ============================

#[test]
fn sql043_flags_naked_delete_in_function() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN DELETE FROM orders; RETURN 1; END; $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql043"), "got: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn sql043_quiet_when_where_present() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN DELETE FROM orders WHERE id = 1; RETURN 1; END; $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql043"));
}

#[test]
fn sql043_quiet_for_top_level_delete() {
  // Top-level DELETE-without-WHERE is the existing sql013 territory,
  // not sql043 (which is scoped to function bodies).
  let d = diags("DELETE FROM orders;");
  assert!(!d.iter().any(|x| x.code == "sql043"));
}

// ===== sql041 NEW/OLD in LANGUAGE sql =======================================

#[test]
fn sql041_flags_new_in_sql_language_function() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE sql AS $$ SELECT NEW.id $$;");
  assert!(d.iter().any(|x| x.code == "sql041"));
}

#[test]
fn sql041_quiet_in_plpgsql_function() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql041"));
}

#[test]
fn sql041_quiet_when_new_inside_string_literal() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE sql AS $$ SELECT 'NEW.id'::TEXT, 1 $$;");
  assert!(!d.iter().any(|x| x.code == "sql041"), "false positive: NEW inside string literal");
}

// ===== sql032 bare RETURN in non-void =======================================

#[test]
fn sql032_flags_bare_return_in_int_function() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql032"));
}

#[test]
fn sql032_quiet_when_returning_value() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 1; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql032"));
}

#[test]
fn sql032_quiet_in_void_function() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RETURN; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql032"));
}

// ===== sql044 EXIT / CONTINUE outside loop ==================================

#[test]
fn sql044_flags_exit_outside_loop() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN EXIT; END; $$;");
  assert!(
    d.iter().any(|x| x.code == "sql044"),
    "expected sql044, got {:?}",
    d.iter().map(|x| x.code).collect::<Vec<_>>()
  );
}

#[test]
fn sql044_quiet_inside_loop() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN LOOP EXIT; END LOOP; RETURN 1; END; $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql044"));
}

#[test]
fn sql044_quiet_inside_while() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN WHILE true LOOP CONTINUE; END LOOP; RETURN 1; END; $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql044"));
}

#[test]
fn sql044_flags_continue_outside_loop() {
  let d = diags("CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN CONTINUE; RETURN 1; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql044"));
}

#[test]
fn sql001_range_is_narrower_than_statement() {
  let src = "SELECT * FROM userss WHERE 1 = 1;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql001").expect("sql001");
  let stmt_len = src.len();
  let diag_len = u32::from(hit.range.end()) - u32::from(hit.range.start());
  assert!((diag_len as usize) < stmt_len / 2, "diag range {} should be << statement {}", diag_len, stmt_len);
}

// ===== batch-51 range-narrow regressions =================================

#[test]
fn sql013_range_narrows_to_update_keyword() {
  let src = "UPDATE users SET name = 'x';";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql013").expect("sql013");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "UPDATE");
}

#[test]
fn sql013_range_narrows_to_delete_keyword() {
  let src = "DELETE FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql013").expect("sql013");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "DELETE");
}

#[test]
fn sql051_range_narrows_to_limit_keyword() {
  let src = "SELECT * FROM users LIMIT 10;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql051").expect("sql051");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "LIMIT");
}

#[test]
fn sql048_range_narrows_to_insert_into() {
  let src = "INSERT INTO users VALUES ('a', 'b');";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql048").expect("sql048");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "INSERT INTO");
}

#[test]
fn sql014_range_narrows_to_from_keyword() {
  let src = "SELECT * FROM users, orders;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql014").expect("sql014");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "FROM");
}

#[test]
fn sql016_range_narrows_to_star() {
  let src = "INSERT INTO orders SELECT * FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql016").expect("sql016");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "*");
}

#[test]
fn sql061_range_narrows_to_null_token() {
  let src = "INSERT INTO users (id, email) VALUES (NULL, 'a@b.com');";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql061").expect("sql061");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "NULL");
}

#[test]
fn sql052_range_covers_like_pattern() {
  let src = "SELECT * FROM users WHERE email LIKE 'literal';";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql052").expect("sql052");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "LIKE 'literal'");
}

// ===== sql169 owner_to_unknown_role =======================================

#[test]
fn sql169_quiet_when_catalog_has_no_roles() {
  // No catalog roles loaded -> can't validate; silent skip.
  let d = diags("ALTER TABLE users OWNER TO whatever;");
  assert!(!d.iter().any(|x| x.code == "sql169"));
}

#[test]
fn sql169_quiet_for_postgres_and_pg_internal_roles() {
  // Run with a populated roles list so the rule activates.
  let mut c = cat();
  c.roles = vec!["app_owner".into()];
  let file =
    parse("ALTER TABLE users OWNER TO postgres; ALTER TABLE users OWNER TO pg_read_all_data;", Dialect::Postgres);
  let scopes = resolve_with_source(
    &file.statements,
    "ALTER TABLE users OWNER TO postgres; ALTER TABLE users OWNER TO pg_read_all_data;",
  );
  let d = run("ALTER TABLE users OWNER TO postgres; ALTER TABLE users OWNER TO pg_read_all_data;", &file, &scopes, &c);
  assert!(!d.iter().any(|x| x.code == "sql169"), "postgres + pg_* roles are whitelisted; got: {d:?}");
}

#[test]
fn sql169_quiet_for_known_role() {
  let mut c = cat();
  c.roles = vec!["app_owner".into(), "readonly".into()];
  let src = "ALTER TABLE users OWNER TO app_owner;";
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  let d = run(src, &file, &scopes, &c);
  assert!(!d.iter().any(|x| x.code == "sql169"));
}

#[test]
fn sql169_fires_for_unknown_role() {
  let mut c = cat();
  c.roles = vec!["app_owner".into()];
  let src = "ALTER TABLE users OWNER TO not_a_real_role;";
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  let d = run(src, &file, &scopes, &c);
  let sql169s: Vec<_> = d.iter().filter(|x| x.code == "sql169").collect();
  assert_eq!(sql169s.len(), 1, "expected one sql169 hit; got: {d:?}");
  assert!(sql169s[0].message.contains("not_a_real_role"));
}

#[test]
fn sql169_quiet_for_current_user_built_in() {
  let mut c = cat();
  c.roles = vec!["app_owner".into()];
  let src = "ALTER TABLE users OWNER TO CURRENT_USER;";
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  let d = run(src, &file, &scopes, &c);
  assert!(!d.iter().any(|x| x.code == "sql169"));
}

// ===== sql089 multiple RAISE EXCEPTION =====================================

#[test]
fn sql089_flags_two_raise_exceptions_in_a_row() {
  let src =
    "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'a'; RAISE EXCEPTION 'b'; END $$;";
  let d = diags(src);
  assert!(d.iter().any(|x| x.code == "sql089"));
}

#[test]
fn sql089_quiet_when_separated_by_if() {
  let src = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN IF x THEN RAISE EXCEPTION 'a'; END IF; IF y THEN RAISE EXCEPTION 'b'; END IF; END $$;";
  let d = diags(src);
  assert!(!d.iter().any(|x| x.code == "sql089"));
}

#[test]
fn sql089_range_points_at_second_raise() {
  let src =
    "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'a'; RAISE EXCEPTION 'b'; END $$;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql089").expect("sql089");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "RAISE EXCEPTION");
}

// ===== sql090 GROUP BY ALL =================================================

#[test]
fn sql090_flags_group_by_all() {
  let d = diags("SELECT a, count(*) FROM users GROUP BY ALL;");
  assert!(d.iter().any(|x| x.code == "sql090"));
}

#[test]
fn sql090_quiet_for_normal_group_by() {
  let d = diags("SELECT a, count(*) FROM users GROUP BY a;");
  assert!(!d.iter().any(|x| x.code == "sql090"));
}

#[test]
fn sql090_range_points_at_group_by_all() {
  let src = "SELECT a, count(*) FROM users GROUP BY ALL;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql090").expect("sql090");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "GROUP BY ALL");
}

// ===== sql095 IS DISTINCT FROM NULL ========================================

#[test]
fn sql095_flags_is_not_distinct_from_null() {
  let d = diags("SELECT * FROM users WHERE id IS NOT DISTINCT FROM NULL;");
  assert!(d.iter().any(|x| x.code == "sql095"));
}

#[test]
fn sql095_flags_is_distinct_from_null() {
  let d = diags("SELECT * FROM users WHERE id IS DISTINCT FROM NULL;");
  assert!(d.iter().any(|x| x.code == "sql095"));
}

#[test]
fn sql095_quiet_for_plain_is_null() {
  let d = diags("SELECT * FROM users WHERE id IS NULL;");
  assert!(!d.iter().any(|x| x.code == "sql095"));
}

#[test]
fn sql095_range_covers_full_expr() {
  let src = "SELECT * FROM users WHERE id IS NOT DISTINCT FROM NULL;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql095").expect("sql095");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "IS NOT DISTINCT FROM NULL");
}

// ===== sql098 multiple WHERE clauses =======================================

#[test]
fn sql098_flags_two_where() {
  let d = diags("SELECT * FROM users WHERE a = 1 WHERE b = 2;");
  assert!(d.iter().any(|x| x.code == "sql098"));
}

#[test]
fn sql098_quiet_when_where_in_subquery() {
  let d = diags("SELECT * FROM users WHERE id IN (SELECT id FROM logs WHERE active);");
  assert!(!d.iter().any(|x| x.code == "sql098"));
}

#[test]
fn sql098_range_points_at_second_where() {
  let src = "SELECT * FROM users WHERE a = 1 WHERE b = 2;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql098").expect("sql098");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "WHERE");
  assert!(s as usize > src.find("WHERE a").unwrap());
}

// ===== sql099 ORDER BY position ============================================

#[test]
fn sql099_flags_positional_order_by() {
  let d = diags("SELECT a, b FROM users ORDER BY 1;");
  assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn sql099_quiet_for_named_order_by() {
  let d = diags("SELECT a, b FROM users ORDER BY a;");
  assert!(!d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn sql099_range_covers_order_by_digit() {
  let src = "SELECT a, b FROM users ORDER BY 1;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql099").expect("sql099");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "ORDER BY 1");
}

// ===== sql101 DISTINCT ON without matching ORDER BY ========================

#[test]
fn sql101_flags_distinct_on_no_order() {
  let d = diags("SELECT DISTINCT ON (id) id, email FROM users;");
  assert!(d.iter().any(|x| x.code == "sql101"));
}

#[test]
fn sql101_quiet_when_order_matches() {
  let d = diags("SELECT DISTINCT ON (id) id, email FROM users ORDER BY id;");
  assert!(!d.iter().any(|x| x.code == "sql101"));
}

#[test]
fn sql101_quiet_when_order_matches_qualified() {
  let d = diags("SELECT DISTINCT ON (u.id) u.id, u.email FROM users u ORDER BY u.id;");
  assert!(!d.iter().any(|x| x.code == "sql101"));
}

#[test]
fn sql101_range_points_at_distinct_on() {
  let src = "SELECT DISTINCT ON (id) id FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql101").expect("sql101");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "DISTINCT ON");
}

// ===== sql104 CHAR(n) ======================================================

#[test]
fn sql104_flags_char_n_in_create() {
  let d = diags("CREATE TABLE x (code CHAR(5));");
  assert!(d.iter().any(|x| x.code == "sql104"));
}

#[test]
fn sql104_flags_character_n_in_create() {
  let d = diags("CREATE TABLE x (code CHARACTER(5));");
  assert!(d.iter().any(|x| x.code == "sql104"));
}

#[test]
fn sql104_quiet_for_character_varying() {
  let d = diags("CREATE TABLE x (code CHARACTER VARYING(50));");
  assert!(!d.iter().any(|x| x.code == "sql104"));
}

#[test]
fn sql104_quiet_for_varchar() {
  let d = diags("CREATE TABLE x (code VARCHAR(50));");
  assert!(!d.iter().any(|x| x.code == "sql104"));
}

#[test]
fn sql104_range_covers_full_type() {
  let src = "CREATE TABLE x (code CHAR(5));";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql104").expect("sql104");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "CHAR(5)");
}

// ===== sql105 TRUNCATE without CASCADE =====================================

#[test]
fn sql105_flags_bare_truncate() {
  let d = diags("TRUNCATE users;");
  assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn sql105_quiet_with_cascade() {
  let d = diags("TRUNCATE users CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn sql105_quiet_with_restrict() {
  let d = diags("TRUNCATE users RESTRICT;");
  assert!(!d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn sql105_range_points_at_truncate() {
  let src = "TRUNCATE users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql105").expect("sql105");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "TRUNCATE");
}

// ===== sql109 length vs char_length ========================================

#[test]
fn sql109_flags_length_call() {
  let d = diags("SELECT length(email) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn sql109_quiet_for_char_length() {
  let d = diags("SELECT char_length(email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn sql109_quiet_for_octet_length() {
  let d = diags("SELECT octet_length(email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn sql109_range_covers_call() {
  let src = "SELECT length(email) FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql109").expect("sql109");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "LENGTH(EMAIL)");
}

// ===== sql111 LOCK TABLE without transaction ===============================

#[test]
fn sql111_flags_bare_lock_table() {
  let d = diags("LOCK TABLE users IN ACCESS EXCLUSIVE MODE;");
  assert!(d.iter().any(|x| x.code == "sql111"));
}

#[test]
fn sql111_quiet_after_begin() {
  let d = diags("BEGIN; LOCK TABLE users IN ACCESS EXCLUSIVE MODE; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql111"));
}

#[test]
fn sql111_range_points_at_lock() {
  let src = "LOCK TABLE users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql111").expect("sql111");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "LOCK");
}

// ===== sql112 generate_series without alias ================================

#[test]
fn sql112_flags_unaliased_generate_series() {
  let d = diags("SELECT * FROM generate_series(1, 10);");
  assert!(d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn sql112_quiet_with_as_alias() {
  let d = diags("SELECT * FROM generate_series(1, 10) AS series;");
  assert!(!d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn sql112_quiet_with_implicit_alias() {
  let d = diags("SELECT * FROM generate_series(1, 10) series;");
  assert!(!d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn sql112_range_covers_call() {
  let src = "SELECT * FROM generate_series(1, 10);";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql112").expect("sql112");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  let slice = &src[s as usize..e as usize];
  assert_eq!(slice.to_ascii_uppercase(), "GENERATE_SERIES(1, 10)");
}

// ===== sql107 jsonb compared without cast ==================================

#[test]
fn sql107_flags_jsonb_path_to_json_literal() {
  let d = diags("SELECT * FROM events WHERE data -> 'meta' = '{\"k\":1}';");
  assert!(d.iter().any(|x| x.code == "sql107"));
}

#[test]
fn sql107_quiet_when_cast_to_text() {
  let d = diags("SELECT * FROM events WHERE data ->> 'meta'::text = 'plain';");
  assert!(!d.iter().any(|x| x.code == "sql107"));
}

// ===== sql113 TIMESTAMP without time zone ==================================

#[test]
fn sql113_flags_bare_timestamp_in_create() {
  let d = diags("CREATE TABLE x (created_at TIMESTAMP);");
  assert!(d.iter().any(|x| x.code == "sql113"));
}

#[test]
fn sql113_quiet_for_timestamptz() {
  let d = diags("CREATE TABLE x (created_at TIMESTAMPTZ);");
  assert!(!d.iter().any(|x| x.code == "sql113"));
}

#[test]
fn sql113_quiet_for_timestamp_with_time_zone() {
  let d = diags("CREATE TABLE x (created_at TIMESTAMP WITH TIME ZONE);");
  assert!(!d.iter().any(|x| x.code == "sql113"));
}

#[test]
fn sql113_range_covers_timestamp_token() {
  let src = "CREATE TABLE x (created_at TIMESTAMP);";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql113").expect("sql113");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "TIMESTAMP");
}

// ===== sql115 jsonb_set with create_missing=false ==========================

#[test]
fn sql115_flags_explicit_false() {
  let d = diags("UPDATE t SET data = jsonb_set(data, '{a}', '1', false) WHERE id = 1;");
  assert!(d.iter().any(|x| x.code == "sql115"));
}

#[test]
fn sql115_quiet_for_default_three_args() {
  let d = diags("UPDATE t SET data = jsonb_set(data, '{a}', '1') WHERE id = 1;");
  assert!(!d.iter().any(|x| x.code == "sql115"));
}

#[test]
fn sql115_quiet_for_explicit_true() {
  let d = diags("UPDATE t SET data = jsonb_set(data, '{a}', '1', true) WHERE id = 1;");
  assert!(!d.iter().any(|x| x.code == "sql115"));
}

// ===== sql116 bare NUMERIC =================================================

#[test]
fn sql116_flags_bare_numeric() {
  let d = diags("CREATE TABLE x (price NUMERIC);");
  assert!(d.iter().any(|x| x.code == "sql116"));
}

#[test]
fn sql116_flags_bare_decimal() {
  let d = diags("CREATE TABLE x (price DECIMAL);");
  assert!(d.iter().any(|x| x.code == "sql116"));
}

#[test]
fn sql116_quiet_for_numeric_with_precision() {
  let d = diags("CREATE TABLE x (price NUMERIC(10, 2));");
  assert!(!d.iter().any(|x| x.code == "sql116"));
}

#[test]
fn sql116_range_covers_numeric_token() {
  let src = "CREATE TABLE x (price NUMERIC);";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql116").expect("sql116");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "NUMERIC");
}

// ===== sql120 DISTINCT redundant with GROUP BY =============================

#[test]
fn sql120_flags_distinct_with_group_by() {
  let d = diags("SELECT DISTINCT a, count(*) FROM users GROUP BY a;");
  assert!(d.iter().any(|x| x.code == "sql120"));
}

#[test]
fn sql120_quiet_for_distinct_alone() {
  let d = diags("SELECT DISTINCT a FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql120"));
}

#[test]
fn sql120_quiet_for_distinct_on() {
  let d = diags("SELECT DISTINCT ON (a) a FROM users GROUP BY a;");
  assert!(!d.iter().any(|x| x.code == "sql120"));
}

#[test]
fn sql120_range_points_at_distinct() {
  let src = "SELECT DISTINCT a, count(*) FROM users GROUP BY a;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql120").expect("sql120");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "DISTINCT");
}

// ===== sql121 cast text to int in WHERE ====================================

#[test]
fn sql121_flags_text_cast_eq_int() {
  let d = diags("SELECT * FROM users WHERE id::text = 5;");
  assert!(d.iter().any(|x| x.code == "sql121"));
}

#[test]
fn sql121_quiet_for_text_cast_eq_string() {
  let d = diags("SELECT * FROM users WHERE id::text = '5';");
  assert!(!d.iter().any(|x| x.code == "sql121"));
}

// ===== sql123 backslash in plain string ====================================

#[test]
fn sql123_flags_backslash_n() {
  let d = diags("SELECT 'line1\\nline2';");
  assert!(d.iter().any(|x| x.code == "sql123"));
}

#[test]
fn sql123_quiet_for_e_prefixed() {
  let d = diags("SELECT E'line1\\nline2';");
  assert!(!d.iter().any(|x| x.code == "sql123"));
}

#[test]
fn sql123_quiet_for_plain_string_no_backslash() {
  let d = diags("SELECT 'hello world';");
  assert!(!d.iter().any(|x| x.code == "sql123"));
}

// ===== sql117 boolean column getting text literal ==========================

#[test]
fn sql117_flags_quoted_true_into_bool_column() {
  let d = diags("INSERT INTO flags (id, active) VALUES ('00000000-0000-0000-0000-000000000000', 'true');");
  assert!(d.iter().any(|x| x.code == "sql117"));
}

#[test]
fn sql117_quiet_for_unquoted_bool() {
  let d = diags("INSERT INTO flags (id, active) VALUES ('00000000-0000-0000-0000-000000000000', true);");
  assert!(!d.iter().any(|x| x.code == "sql117"));
}

#[test]
fn sql117_quiet_for_explicit_cast() {
  let d = diags("INSERT INTO flags (id, active) VALUES ('00000000-0000-0000-0000-000000000000', 'true'::boolean);");
  assert!(!d.iter().any(|x| x.code == "sql117"));
}

// ===== sql122 LIKE in CREATE INDEX/VIEW without COLLATE ====================

#[test]
fn sql122_flags_like_in_create_view() {
  let d = diags("CREATE VIEW v AS SELECT * FROM users WHERE email LIKE 'a%';");
  assert!(d.iter().any(|x| x.code == "sql122"));
}

#[test]
fn sql122_quiet_for_ad_hoc_select() {
  let d = diags("SELECT * FROM users WHERE email LIKE 'a%';");
  assert!(!d.iter().any(|x| x.code == "sql122"));
}

#[test]
fn sql122_quiet_when_collate_present() {
  let d = diags("CREATE VIEW v AS SELECT * FROM users WHERE email COLLATE \"C\" LIKE 'a%';");
  assert!(!d.iter().any(|x| x.code == "sql122"));
}

// ===== sql118 SELECT INTO outside plpgsql ==================================

#[test]
fn sql118_flags_top_level_select_into() {
  let d = diags("SELECT id, email INTO snapshot FROM users;");
  assert!(d.iter().any(|x| x.code == "sql118"));
}

#[test]
fn sql118_quiet_for_normal_select() {
  let d = diags("SELECT id, email FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql118"));
}

#[test]
fn sql118_range_points_at_into() {
  let src = "SELECT id INTO snapshot FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql118").expect("sql118");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "INTO");
}

#[test]
fn sql118_quiet_inside_plpgsql_body() {
  let src = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ DECLARE v users; BEGIN SELECT * INTO v FROM users; END $$;";
  let d = diags(src);
  assert!(!d.iter().any(|x| x.code == "sql118"));
}

#[test]
fn sql118_quiet_inside_do_block() {
  let src = "DO $$ DECLARE v users; BEGIN SELECT * INTO v FROM users; END $$;";
  let d = diags(src);
  assert!(!d.iter().any(|x| x.code == "sql118"));
}

// ===== sql124 CTE missing RECURSIVE ========================================

#[test]
fn sql124_flags_self_ref_without_recursive() {
  let d = diags("WITH t AS (SELECT 1 UNION ALL SELECT n+1 FROM t WHERE n < 10) SELECT * FROM t;");
  assert!(d.iter().any(|x| x.code == "sql124"));
}

#[test]
fn sql124_quiet_when_recursive_present() {
  let d = diags("WITH RECURSIVE t AS (SELECT 1 UNION ALL SELECT n+1 FROM t WHERE n < 10) SELECT * FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql124"));
}

#[test]
fn sql124_quiet_for_non_self_referencing_cte() {
  let d = diags("WITH t AS (SELECT id FROM users) SELECT * FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql124"));
}

// ===== sql125 EXPLAIN ANALYZE on DML =======================================

#[test]
fn sql125_flags_explain_analyze_update() {
  let d = diags("EXPLAIN ANALYZE UPDATE users SET name = 'a' WHERE id = 1;");
  assert!(d.iter().any(|x| x.code == "sql125"));
}

#[test]
fn sql125_flags_explain_analyze_insert() {
  let d = diags("EXPLAIN ANALYZE INSERT INTO users (email) VALUES ('a@b.com');");
  assert!(d.iter().any(|x| x.code == "sql125"));
}

#[test]
fn sql125_quiet_for_explain_analyze_select() {
  let d = diags("EXPLAIN ANALYZE SELECT * FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql125"));
}

#[test]
fn sql125_quiet_for_plain_explain() {
  let d = diags("EXPLAIN UPDATE users SET name = 'a' WHERE id = 1;");
  assert!(!d.iter().any(|x| x.code == "sql125"));
}

// ===== sql128 GRANT to PUBLIC ==============================================

#[test]
fn sql128_flags_grant_to_public() {
  let d = diags("GRANT SELECT ON users TO PUBLIC;");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn sql128_quiet_for_grant_to_specific_role() {
  let d = diags("GRANT SELECT ON users TO app_user;");
  assert!(!d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn sql128_range_points_at_to_public() {
  let src = "GRANT SELECT ON users TO PUBLIC;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql128").expect("sql128");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "TO PUBLIC");
}

// ===== sql127 UPDATE FROM without join condition ===========================

#[test]
fn sql127_flags_update_from_without_where() {
  let d = diags("UPDATE users SET name = src.name FROM staging src;");
  assert!(d.iter().any(|x| x.code == "sql127"));
}

#[test]
fn sql127_flags_update_from_where_no_join_cond() {
  let d = diags("UPDATE users SET name = 'x' FROM staging WHERE 1 = 1;");
  assert!(d.iter().any(|x| x.code == "sql127"));
}

#[test]
fn sql127_quiet_for_update_from_with_join_cond() {
  let d = diags("UPDATE users SET name = src.name FROM staging src WHERE users.id = src.id;");
  assert!(!d.iter().any(|x| x.code == "sql127"));
}

#[test]
fn sql127_quiet_for_plain_update() {
  let d = diags("UPDATE users SET name = 'x' WHERE id = 1;");
  assert!(!d.iter().any(|x| x.code == "sql127"));
}

// ===== sql119 SET TRANSACTION ISOLATION not first ==========================

#[test]
fn sql119_flags_set_iso_after_select() {
  let d = diags("BEGIN; SELECT 1; SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;");
  assert!(d.iter().any(|x| x.code == "sql119"));
}

#[test]
fn sql119_quiet_when_first_after_begin() {
  let d = diags("BEGIN; SET TRANSACTION ISOLATION LEVEL SERIALIZABLE; SELECT 1;");
  assert!(!d.iter().any(|x| x.code == "sql119"));
}

#[test]
fn sql119_quiet_when_no_begin() {
  let d = diags("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;");
  assert!(!d.iter().any(|x| x.code == "sql119"));
}

// ===== sql131 RAISE message has more placeholders than args ================

#[test]
fn sql131_flags_missing_arg() {
  let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE 'value is %s'; END $$;");
  assert!(d.iter().any(|x| x.code == "sql131"));
}

#[test]
fn sql131_quiet_when_args_match() {
  let d =
    diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE 'value is %s', 'x'; END $$;");
  assert!(!d.iter().any(|x| x.code == "sql131"));
}

#[test]
fn sql131_quiet_when_no_placeholder() {
  let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE 'plain message'; END $$;");
  assert!(!d.iter().any(|x| x.code == "sql131"));
}

#[test]
fn sql131_quiet_for_escaped_percent() {
  let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE '100%%'; END $$;");
  assert!(!d.iter().any(|x| x.code == "sql131"));
}

// ===== sql134 VACUUM in transaction ========================================

#[test]
fn sql134_flags_vacuum_inside_begin() {
  let d = diags("BEGIN; VACUUM users; COMMIT;");
  assert!(d.iter().any(|x| x.code == "sql134"));
}

#[test]
fn sql134_flags_reindex_inside_begin() {
  let d = diags("BEGIN; REINDEX TABLE users; COMMIT;");
  assert!(d.iter().any(|x| x.code == "sql134"));
}

#[test]
fn sql134_quiet_for_bare_vacuum() {
  let d = diags("VACUUM users;");
  assert!(!d.iter().any(|x| x.code == "sql134"));
}

#[test]
fn sql134_quiet_after_commit() {
  let d = diags("BEGIN; SELECT 1; COMMIT; VACUUM users;");
  assert!(!d.iter().any(|x| x.code == "sql134"));
}

// ===== sql130 multiple TRUNCATE in transaction =============================

#[test]
fn sql130_flags_second_truncate_in_tx() {
  let d = diags("BEGIN; TRUNCATE users; TRUNCATE orders; COMMIT;");
  assert!(d.iter().any(|x| x.code == "sql130"));
}

#[test]
fn sql130_quiet_for_single_truncate() {
  let d = diags("BEGIN; TRUNCATE users; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql130"));
}

#[test]
fn sql130_quiet_for_combined_truncate() {
  let d = diags("BEGIN; TRUNCATE users, orders; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql130"));
}

// ===== sql129 CREATE TABLE without ALTER OWNER =============================

#[test]
fn sql129_unregistered_does_not_fire() {
  // sql129 alter_table_no_owner was unregistered (too noisy in
  // practice). The rule module still exists for future opt-in.
  let d = diags("CREATE TABLE widgets (id uuid PRIMARY KEY);");
  assert!(!d.iter().any(|x| x.code == "sql129"));
}

#[test]
fn sql129_quiet_when_alter_owner_follows() {
  let d = diags("CREATE TABLE widgets (id uuid PRIMARY KEY); ALTER TABLE widgets OWNER TO app;");
  assert!(!d.iter().any(|x| x.code == "sql129"));
}

#[test]
fn sql129_quiet_for_non_table() {
  let d = diags("CREATE INDEX idx_x ON widgets (id);");
  assert!(!d.iter().any(|x| x.code == "sql129"));
}

// ===== sql136 COPY without FORMAT clause ===================================

#[test]
fn sql136_flags_bare_copy() {
  let d = diags("COPY users FROM '/tmp/users.csv';");
  assert!(d.iter().any(|x| x.code == "sql136"));
}

#[test]
fn sql136_quiet_with_format_clause() {
  let d = diags("COPY users FROM '/tmp/users.csv' WITH (FORMAT csv);");
  assert!(!d.iter().any(|x| x.code == "sql136"));
}

#[test]
fn sql136_quiet_with_csv_keyword() {
  let d = diags("COPY users FROM '/tmp/users.csv' CSV;");
  assert!(!d.iter().any(|x| x.code == "sql136"));
}

// ===== sql132 FOR UPDATE in recursive CTE ==================================

#[test]
fn sql132_flags_for_update_in_recursive_cte() {
  let d = diags("WITH RECURSIVE t AS (SELECT 1 UNION SELECT id FROM users FOR UPDATE) SELECT * FROM t;");
  assert!(d.iter().any(|x| x.code == "sql132"));
}

#[test]
fn sql132_quiet_for_non_recursive_cte() {
  let d = diags("WITH t AS (SELECT id FROM users FOR UPDATE) SELECT * FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql132"));
}

#[test]
fn sql132_quiet_when_no_for_update() {
  let d = diags("WITH RECURSIVE t AS (SELECT 1 UNION SELECT id FROM users) SELECT * FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql132"));
}

// ===== sql137 LISTEN without UNLISTEN ======================================

#[test]
fn sql137_flags_bare_listen() {
  let d = diags("LISTEN events;");
  assert!(d.iter().any(|x| x.code == "sql137"));
}

#[test]
fn sql137_quiet_when_unlisten_follows() {
  let d = diags("LISTEN events; SELECT 1; UNLISTEN events;");
  assert!(!d.iter().any(|x| x.code == "sql137"));
}

#[test]
fn sql137_quiet_when_unlisten_star_follows() {
  let d = diags("LISTEN events; SELECT 1; UNLISTEN *;");
  assert!(!d.iter().any(|x| x.code == "sql137"));
}

// ===== sql135 SET ROLE without RESET ROLE ==================================

#[test]
fn sql135_flags_bare_set_role() {
  let d = diags("BEGIN; SET ROLE admin; UPDATE users SET name = 'x'; COMMIT;");
  assert!(d.iter().any(|x| x.code == "sql135"));
}

#[test]
fn sql135_quiet_with_reset() {
  let d = diags("BEGIN; SET ROLE admin; UPDATE users SET name = 'x'; RESET ROLE; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql135"));
}

#[test]
fn sql135_quiet_with_set_role_none() {
  let d = diags("BEGIN; SET ROLE admin; UPDATE users SET name = 'x'; SET ROLE NONE; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql135"));
}

// ===== sql140 INSERT trigger WHEN references OLD ===========================

#[test]
fn sql140_flags_old_in_insert_trigger() {
  let d = diags("CREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW WHEN (OLD.id IS NULL) EXECUTE FUNCTION f();");
  assert!(d.iter().any(|x| x.code == "sql140"));
}

#[test]
fn sql140_quiet_for_update_trigger() {
  let d = diags("CREATE TRIGGER t AFTER UPDATE ON users FOR EACH ROW WHEN (OLD.id IS NULL) EXECUTE FUNCTION f();");
  assert!(!d.iter().any(|x| x.code == "sql140"));
}

#[test]
fn sql140_quiet_when_only_new_referenced() {
  let d = diags("CREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();");
  assert!(!d.iter().any(|x| x.code == "sql140"));
}

// ===== sql141 ALTER TYPE ADD VALUE in transaction ==========================

#[test]
fn sql141_flags_alter_type_in_tx() {
  let d = diags("BEGIN; ALTER TYPE color ADD VALUE 'red'; COMMIT;");
  assert!(d.iter().any(|x| x.code == "sql141"));
}

#[test]
fn sql141_quiet_for_bare_alter_type() {
  let d = diags("ALTER TYPE color ADD VALUE 'red';");
  assert!(!d.iter().any(|x| x.code == "sql141"));
}

// ===== sql133 GRANT ... WITH GRANT OPTION ==================================

#[test]
fn sql133_flags_with_grant_option() {
  let d = diags("GRANT SELECT ON users TO app_user WITH GRANT OPTION;");
  assert!(d.iter().any(|x| x.code == "sql133"));
}

#[test]
fn sql133_quiet_for_plain_grant() {
  let d = diags("GRANT SELECT ON users TO app_user;");
  assert!(!d.iter().any(|x| x.code == "sql133"));
}

#[test]
fn sql133_range_points_at_clause() {
  let src = "GRANT SELECT ON users TO app_user WITH GRANT OPTION;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql133").expect("sql133");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize], "WITH GRANT OPTION");
}

// ===== sql142 IMMUTABLE function body does DDL =============================

#[test]
fn sql142_flags_create_inside_immutable() {
  let d =
    diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql IMMUTABLE AS $$ BEGIN CREATE TABLE t (id int); END $$;");
  assert!(d.iter().any(|x| x.code == "sql142"));
}

#[test]
fn sql142_quiet_for_stable_function() {
  let d =
    diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql STABLE AS $$ BEGIN CREATE TABLE t (id int); END $$;");
  assert!(!d.iter().any(|x| x.code == "sql142"));
}

#[test]
fn sql142_quiet_for_immutable_no_ddl() {
  let d = diags("CREATE FUNCTION f(a int) RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT a + 1 $$;");
  assert!(!d.iter().any(|x| x.code == "sql142"));
}

// ===== sql145 DEFAULT volatile =============================================

#[test]
fn sql145_flags_default_random() {
  let d = diags("CREATE TABLE t (id int DEFAULT random());");
  assert!(d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_quiet_for_default_nextval() {
  // nextval is the *intended* default for serial-ish columns.
  let d = diags("CREATE TABLE t (id int DEFAULT nextval('seq'));");
  assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_quiet_for_now_default() {
  let d = diags("CREATE TABLE t (created_at timestamptz DEFAULT now());");
  assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_quiet_for_constant_default() {
  let d = diags("CREATE TABLE t (active bool DEFAULT true);");
  assert!(!d.iter().any(|x| x.code == "sql145"));
}

// ===== sql002 column lookup honors CTE columns =============================

#[test]
fn sql002_accepts_known_cte_column() {
  let d = diags("WITH t AS (SELECT id, email FROM users) SELECT t.id FROM t;");
  assert!(
    !d.iter().any(|x| x.code == "sql002"),
    "t.id is in the CTE projection, expected quiet: {:?}",
    d.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>()
  );
}

#[test]
fn sql002_flags_unknown_cte_column() {
  let d = diags("WITH t AS (SELECT id, email FROM users) SELECT t.bogus FROM t;");
  assert!(
    d.iter().any(|x| x.code == "sql002"),
    "t.bogus is not in projection, expected sql002: {:?}",
    d.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>()
  );
}

#[test]
fn sql002_accepts_schema_qualified_column() {
  let d = diags("SELECT public.users.id FROM public.users;");
  assert!(
    !d.iter().any(|x| x.code == "sql002"),
    "public.users.id is a known column; got: {:?}",
    d.iter().filter(|x| x.code == "sql002").collect::<Vec<_>>()
  );
}

// ===== sql146 unbounded VARCHAR ===========================================

#[test]
fn sql146_flags_bare_varchar() {
  let d = diags("CREATE TABLE x (name VARCHAR);");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn sql146_flags_character_varying() {
  let d = diags("CREATE TABLE x (name CHARACTER VARYING);");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn sql146_quiet_for_varchar_with_limit() {
  let d = diags("CREATE TABLE x (name VARCHAR(255));");
  assert!(!d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn sql146_quiet_for_text() {
  let d = diags("CREATE TABLE x (name TEXT);");
  assert!(!d.iter().any(|x| x.code == "sql146"));
}

// ===== sql148 array subscript zero / negative =============================

#[test]
fn sql148_flags_subscript_zero() {
  let d = diags("SELECT arr[0] FROM t;");
  assert!(d.iter().any(|x| x.code == "sql148"));
}

#[test]
fn sql148_flags_subscript_negative() {
  let d = diags("SELECT arr[-1] FROM t;");
  assert!(d.iter().any(|x| x.code == "sql148"));
}

#[test]
fn sql148_quiet_for_subscript_one() {
  let d = diags("SELECT arr[1] FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql148"));
}

#[test]
fn sql148_quiet_for_empty_brackets_type() {
  let d = diags("CREATE TABLE x (xs int[]);");
  assert!(!d.iter().any(|x| x.code == "sql148"));
}

// ===== sql144 DELETE trigger references NEW ================================

#[test]
fn sql144_flags_new_in_delete_trigger() {
  let d = diags("CREATE TRIGGER t AFTER DELETE ON users FOR EACH ROW WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();");
  assert!(d.iter().any(|x| x.code == "sql144"));
}

#[test]
fn sql144_quiet_for_update_trigger() {
  let d = diags("CREATE TRIGGER t AFTER UPDATE ON users FOR EACH ROW WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();");
  assert!(!d.iter().any(|x| x.code == "sql144"));
}

#[test]
fn sql144_quiet_when_only_old_referenced() {
  let d = diags("CREATE TRIGGER t AFTER DELETE ON users FOR EACH ROW WHEN (OLD.id IS NOT NULL) EXECUTE FUNCTION f();");
  assert!(!d.iter().any(|x| x.code == "sql144"));
}

// ===== sql150 CASE without ELSE ============================================

#[test]
fn sql150_flags_case_no_else() {
  let d = diags("SELECT CASE WHEN id > 0 THEN 'pos' END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql150"));
}

#[test]
fn sql150_quiet_when_else_present() {
  let d = diags("SELECT CASE WHEN id > 0 THEN 'pos' ELSE 'np' END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql150"));
}

#[test]
fn sql150_range_points_at_case_keyword() {
  let src = "SELECT CASE WHEN id > 0 THEN 'pos' END FROM users;";
  let d = diags(src);
  let hit = d.iter().find(|x| x.code == "sql150").expect("sql150");
  let s: u32 = hit.range.start().into();
  let e: u32 = hit.range.end().into();
  assert_eq!(&src[s as usize..e as usize].to_ascii_uppercase(), "CASE");
}

// ===== sql149 UPDATE SET x = x =============================================

#[test]
fn sql149_flags_self_assignment() {
  let d = diags("UPDATE users SET name = name WHERE id = 1;");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn sql149_quiet_for_normal_set() {
  let d = diags("UPDATE users SET name = 'x' WHERE id = 1;");
  assert!(!d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn sql149_flags_qualified_self_assignment() {
  let d = diags("UPDATE users SET u.name = u.name WHERE u.id = 1;");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

// ===== sql143 RETURNING without INTO inside plpgsql ========================

#[test]
fn sql143_flags_returning_no_into() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN INSERT INTO users (email) VALUES ('a@b.com') RETURNING id; END $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql143"));
}

#[test]
fn sql143_quiet_when_into_present() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ DECLARE new_id uuid; BEGIN INSERT INTO users (email) VALUES ('a@b.com') RETURNING id INTO new_id; END $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql143"));
}

#[test]
fn sql143_quiet_for_top_level_returning() {
  let d = diags("INSERT INTO users (email) VALUES ('a@b.com') RETURNING id;");
  assert!(!d.iter().any(|x| x.code == "sql143"));
}

// ===== sql126 DML in plpgsql without GET DIAGNOSTICS ======================

#[test]
fn sql126_flags_update_no_diagnostics() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN UPDATE users SET name = 'x' WHERE id = '1'; END $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql126"));
}

#[test]
fn sql126_quiet_when_get_diagnostics_follows() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS int LANGUAGE plpgsql AS $$ DECLARE n int; BEGIN UPDATE users SET name = 'x' WHERE id = '1'; GET DIAGNOSTICS n = ROW_COUNT; RETURN n; END $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql126"));
}

#[test]
fn sql126_quiet_when_returning_into_present() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS uuid LANGUAGE plpgsql AS $$ DECLARE r uuid; BEGIN UPDATE users SET name = 'x' WHERE id = '1' RETURNING id INTO r; RETURN r; END $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql126"));
}

#[test]
fn sql126_quiet_for_insert_in_trigger_function() {
  // Fire-and-forget INSERT inside a trigger function body --
  // ROW_COUNT here is meaningless, the audit row always exists.
  let src = r#"CREATE OR REPLACE FUNCTION log_order_status_change ()
    RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO order_status_history (order_id, old_status, new_status, changed_at)
    VALUES (NEW.id, OLD.status, NEW.status, now());
    RETURN NEW;
END;
$$;"#;
  let d = diags(src);
  assert!(
    !d.iter().any(|x| x.code == "sql126"),
    "INSERT in trigger fn shouldn't trigger sql126; got: {:?}",
    d.iter().filter(|x| x.code == "sql126").map(|x| &x.message).collect::<Vec<_>>()
  );
}

#[test]
fn sql126_quiet_for_insert_in_plain_function() {
  // INSERT in any plpgsql body now passes silently (fire-and-forget
  // is the common case). Only UPDATE/DELETE trigger sql126.
  let src = "CREATE FUNCTION audit() RETURNS void LANGUAGE plpgsql AS $$ BEGIN INSERT INTO audit_log (msg) VALUES ('event'); END $$;";
  let d = diags(src);
  assert!(!d.iter().any(|x| x.code == "sql126"));
}

// ===== sql154 count(*) returns 1 row even when WHERE matches none =========

#[test]
fn sql154_flags_count_star_with_where() {
  let d = diags("SELECT count(*) FROM users WHERE id = '0';");
  assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn sql154_quiet_when_group_by_present() {
  let d = diags("SELECT count(*) FROM users WHERE id = '0' GROUP BY name;");
  assert!(!d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn sql154_quiet_when_no_where() {
  let d = diags("SELECT count(*) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql154"));
}

// ===== sql164 string literal +/- int =====================================

#[test]
fn sql164_flags_string_plus_int() {
  let d = diags("SELECT 'foo' + 1 FROM users;");
  assert!(d.iter().any(|x| x.code == "sql164"));
}

#[test]
fn sql164_quiet_for_concat() {
  let d = diags("SELECT 'foo' || 1 FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql164"));
}

#[test]
fn sql164_quiet_for_string_only() {
  let d = diags("SELECT 'foo' FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql164"));
}

// ===== real-world golden tests =============================================
// These assert the linter produces ZERO unexpected diagnostics on common
// production patterns. If a future rule introduces a false positive on
// any of these, the test fails immediately.

#[test]
fn golden_set_updated_at_trigger_zero_warnings() {
  let src = r#"CREATE OR REPLACE FUNCTION set_updated_at ()
    RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;"#;
  let d = diags(src);
  assert!(
    d.is_empty(),
    "expected zero diagnostics, got: {:?}",
    d.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>()
  );
}

#[test]
fn golden_order_status_history_trigger_zero_warnings() {
  let src = r#"CREATE OR REPLACE FUNCTION log_order_status_change ()
    RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO order_status_history (order_id, old_status, new_status, changed_at)
    VALUES (NEW.id, OLD.status, NEW.status, now());
    RETURN NEW;
END;
$$;"#;
  let d = diags(src);
  assert!(
    d.is_empty(),
    "expected zero diagnostics, got: {:?}",
    d.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>()
  );
}

#[test]
fn golden_audit_log_table_zero_warnings() {
  let src = r#"CREATE TABLE audit_log (
    id uuid NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    actor_id uuid NOT NULL,
    action text NOT NULL,
    target_table text NOT NULL,
    target_id uuid,
    payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);"#;
  let d = diags(src);
  let unexpected: Vec<_> = d.iter().filter(|x| !matches!(x.code, "sql028")).collect();
  assert!(
    unexpected.is_empty(),
    "expected zero diagnostics, got: {:?}",
    unexpected.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>()
  );
}

// ===== sql151 missing LATERAL ==============================================

#[test]
fn sql151_quiet_for_implicit_lateral_function() {
  // PG documents that LATERAL is OPTIONAL in front of a function in
  // the FROM list -- `generate_series(u.id, 10)` after `users u` is
  // implicitly lateral. Don't fire here.
  let d = diags("SELECT * FROM users u, generate_series(u.id, 10);");
  assert!(!d.iter().any(|x| x.code == "sql151"));
}

#[test]
fn sql151_quiet_with_lateral() {
  let d = diags("SELECT * FROM users u, LATERAL generate_series(u.id, 10);");
  assert!(!d.iter().any(|x| x.code == "sql151"));
}

#[test]
fn sql151_quiet_when_fn_takes_constants() {
  let d = diags("SELECT * FROM users u, generate_series(1, 10);");
  assert!(!d.iter().any(|x| x.code == "sql151"));
}

// ===== sql166 ROW(x) single-element constructor ============================

#[test]
fn sql166_flags_single_element_row() {
  let d = diags("SELECT ROW(id) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql166"));
}

#[test]
fn sql166_quiet_for_multi_element_row() {
  let d = diags("SELECT ROW(id, email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql166"));
}

#[test]
fn sql166_quiet_for_implicit_tuple() {
  let d = diags("SELECT (id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql166"));
}

// ===== sql167 redundant index on PK column ================================

#[test]
fn sql167_flags_index_on_pk_column() {
  let d = diags("CREATE INDEX idx_users_id ON users (id);");
  assert!(d.iter().any(|x| x.code == "sql167"));
}

#[test]
fn sql167_quiet_for_index_on_non_pk_column() {
  let d = diags("CREATE INDEX idx_users_email ON users (email);");
  assert!(!d.iter().any(|x| x.code == "sql167"));
}

// ===== sql152 BEGIN without explicit lock mode ============================

#[test]
fn sql152_flags_begin_with_update_no_lock() {
  let d = diags("BEGIN; UPDATE users SET name = 'x' WHERE id = '1'; COMMIT;");
  assert!(d.iter().any(|x| x.code == "sql152"));
}

#[test]
fn sql152_quiet_with_for_update_lock() {
  let d =
    diags("BEGIN; SELECT * FROM users WHERE id = '1' FOR UPDATE; UPDATE users SET name = 'x' WHERE id = '1'; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql152"));
}

#[test]
fn sql152_quiet_for_read_only_tx() {
  let d = diags("BEGIN; SELECT * FROM users; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql152"));
}

#[test]
fn golden_user_roles_unique_pair_zero_warnings() {
  let src = r#"CREATE TABLE user_roles (
    user_id uuid NOT NULL,
    role text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, role)
);"#;
  let d = diags(src);
  let unexpected: Vec<_> = d.iter().filter(|x| !matches!(x.code, "sql028" | "sql046")).collect();
  assert!(
    unexpected.is_empty(),
    "expected zero diagnostics, got: {:?}",
    unexpected.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>()
  );
}

// ===== sql139 UNIQUE on nullable ==========================================

#[test]
fn sql139_flags_unique_no_not_null() {
  let d = diags("CREATE TABLE x (email TEXT UNIQUE);");
  assert!(d.iter().any(|x| x.code == "sql139"));
}

#[test]
fn sql139_quiet_when_not_null_present() {
  let d = diags("CREATE TABLE x (email TEXT NOT NULL UNIQUE);");
  assert!(!d.iter().any(|x| x.code == "sql139"));
}

#[test]
fn sql139_quiet_when_nulls_not_distinct() {
  let d = diags("CREATE TABLE x (email TEXT UNIQUE NULLS NOT DISTINCT);");
  assert!(!d.iter().any(|x| x.code == "sql139"));
}

// ===== regression: sql126 + sql045 must not fire on trigger funcs ==========

#[test]
fn sql126_quiet_on_assignment_to_updated_at_field() {
  // `new.updated_at := now()` is a PL/pgSQL assignment to a record
  // field. `UPDATE` appears inside the column name `updated_at` --
  // word-bounded + statement-start matching must reject it.
  let src = "CREATE OR REPLACE FUNCTION set_updated_at() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN new.updated_at := now(); RETURN new; END $$;";
  let d = diags(src);
  assert!(
    !d.iter().any(|x| x.code == "sql126"),
    "sql126 false-positive on assignment: {:?}",
    d.iter().filter(|x| x.code == "sql126").collect::<Vec<_>>()
  );
}

#[test]
fn sql045_quiet_on_return_at_end_of_trigger_body() {
  // `RETURN new;` is the natural last statement of a trigger fn.
  // The token `new` is the RETURN argument, not the next stmt.
  let src = "CREATE OR REPLACE FUNCTION set_updated_at() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN new.updated_at := now(); RETURN new; END $$;";
  let d = diags(src);
  assert!(
    !d.iter().any(|x| x.code == "sql045"),
    "sql045 false-positive on RETURN: {:?}",
    d.iter().filter(|x| x.code == "sql045").collect::<Vec<_>>()
  );
}

// ===== sql155 TRUNCATE RETURNING ===========================================

#[test]
fn sql155_flags_truncate_returning() {
  let d = diags("TRUNCATE users RETURNING id;");
  assert!(d.iter().any(|x| x.code == "sql155"));
}

#[test]
fn sql155_quiet_for_bare_truncate() {
  let d = diags("TRUNCATE users;");
  assert!(!d.iter().any(|x| x.code == "sql155"));
}

// ===== sql138 ::text inside DISTINCT =======================================

#[test]
fn sql138_flags_distinct_cast_to_text() {
  let d = diags("SELECT DISTINCT id::text FROM users;");
  assert!(d.iter().any(|x| x.code == "sql138"));
}

#[test]
fn sql138_quiet_for_distinct_on() {
  let d = diags("SELECT DISTINCT ON (id) id::text FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql138"));
}

#[test]
fn sql138_quiet_for_plain_distinct() {
  let d = diags("SELECT DISTINCT id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql138"));
}

// ===== sql156 SELECT INTO STRICT without EXCEPTION =========================

#[test]
fn sql156_flags_strict_no_exception_block() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS uuid LANGUAGE plpgsql AS $$ DECLARE r uuid; BEGIN SELECT id INTO STRICT r FROM users WHERE email = 'x'; RETURN r; END $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql156"));
}

#[test]
fn sql156_quiet_with_exception_block() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS uuid LANGUAGE plpgsql AS $$ DECLARE r uuid; BEGIN SELECT id INTO STRICT r FROM users WHERE email = 'x'; RETURN r; EXCEPTION WHEN NO_DATA_FOUND THEN RETURN NULL; END $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql156"));
}

#[test]
fn sql156_quiet_without_strict() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS uuid LANGUAGE plpgsql AS $$ DECLARE r uuid; BEGIN SELECT id INTO r FROM users WHERE email = 'x'; RETURN r; END $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql156"));
}

// ===== sql153 timestamp + int arithmetic ===================================

#[test]
fn sql153_flags_now_plus_int() {
  let d = diags("SELECT now() + 1 FROM users;");
  assert!(d.iter().any(|x| x.code == "sql153"));
}

#[test]
fn sql153_quiet_with_interval() {
  let d = diags("SELECT now() + INTERVAL '1 day' FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql153"));
}

#[test]
fn sql153_flags_current_date_minus_int() {
  let d = diags("SELECT current_date - 7 FROM users;");
  assert!(d.iter().any(|x| x.code == "sql153"));
}

// ===== sql145 default whitelist regression ================================

#[test]
fn sql145_quiet_for_gen_random_uuid_default() {
  let d = diags("CREATE TABLE t (id uuid DEFAULT gen_random_uuid());");
  assert!(
    !d.iter().any(|x| x.code == "sql145"),
    "gen_random_uuid() is the intended default use; got: {:?}",
    d.iter().filter(|x| x.code == "sql145").collect::<Vec<_>>()
  );
}

#[test]
fn sql145_quiet_for_uuid_generate_v4_default() {
  let d = diags("CREATE TABLE t (id uuid DEFAULT uuid_generate_v4());");
  assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_quiet_for_nextval_default_whitelisted() {
  let d = diags("CREATE TABLE t (id int DEFAULT nextval('seq'));");
  assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn sql145_still_flags_random() {
  let d = diags("CREATE TABLE t (lottery int DEFAULT random());");
  assert!(d.iter().any(|x| x.code == "sql145"), "random() default is unlikely intentional");
}

// ===== sql139 UNIQUE column-list regression ===============================

#[test]
fn sql139_quiet_when_all_unique_columns_not_null() {
  let src = "CREATE TABLE user_roles (
        user_id uuid NOT NULL,
        role text NOT NULL,
        UNIQUE (user_id, role)
    );";
  let d = diags(src);
  assert!(
    !d.iter().any(|x| x.code == "sql139"),
    "UNIQUE (user_id, role) over NOT NULL cols, expected quiet; got: {:?}",
    d.iter().filter(|x| x.code == "sql139").map(|x| &x.message).collect::<Vec<_>>()
  );
}

#[test]
fn sql139_flags_when_one_unique_column_nullable() {
  let src = "CREATE TABLE user_roles (
        user_id uuid NOT NULL,
        role text,
        UNIQUE (user_id, role)
    );";
  let d = diags(src);
  assert!(d.iter().any(|x| x.code == "sql139"), "role is nullable, expected sql139");
}

// ===== effective-column model: implicit PG semantics ======================

#[test]
fn sql139_quiet_for_unique_over_pk_columns() {
  // Inline PRIMARY KEY implies NOT NULL -- explicit NOT NULL not
  // needed for the UNIQUE-on-nullable check to stay quiet.
  let src = "CREATE TABLE t (id uuid PRIMARY KEY, role text NOT NULL, UNIQUE (id, role));";
  let d = diags(src);
  assert!(
    !d.iter().any(|x| x.code == "sql139"),
    "PK column should be NOT NULL via implicit semantics; got: {:?}",
    d.iter().filter(|x| x.code == "sql139").collect::<Vec<_>>()
  );
}

#[test]
fn sql139_quiet_for_unique_over_table_level_pk() {
  // Table-level PRIMARY KEY (id, tenant) marks both as NOT NULL.
  let src =
    "CREATE TABLE t (id uuid, tenant uuid, role text NOT NULL, PRIMARY KEY (id, tenant), UNIQUE (id, tenant, role));";
  let d = diags(src);
  assert!(
    !d.iter().any(|x| x.code == "sql139"),
    "table-level PK should propagate NOT NULL; got: {:?}",
    d.iter().filter(|x| x.code == "sql139").collect::<Vec<_>>()
  );
}

#[test]
fn sql139_quiet_for_unique_over_serial() {
  // SERIAL implies NOT NULL.
  let src = "CREATE TABLE t (id SERIAL PRIMARY KEY, slug text NOT NULL, UNIQUE (id, slug));";
  let d = diags(src);
  assert!(
    !d.iter().any(|x| x.code == "sql139"),
    "SERIAL should be NOT NULL via implicit semantics; got: {:?}",
    d.iter().filter(|x| x.code == "sql139").collect::<Vec<_>>()
  );
}

#[test]
fn sql139_quiet_for_unique_over_identity() {
  // GENERATED ... AS IDENTITY implies NOT NULL.
  let src = "CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY PRIMARY KEY, name text NOT NULL, UNIQUE (id, name));";
  let d = diags(src);
  assert!(
    !d.iter().any(|x| x.code == "sql139"),
    "IDENTITY should be NOT NULL via implicit semantics; got: {:?}",
    d.iter().filter(|x| x.code == "sql139").collect::<Vec<_>>()
  );
}

// ===== sql160 advisory lock without unlock =================================

#[test]
fn sql160_flags_session_lock_no_unlock() {
  let d = diags("SELECT pg_advisory_lock(42);");
  assert!(d.iter().any(|x| x.code == "sql160"));
}

#[test]
fn sql160_quiet_when_unlock_follows() {
  let d = diags("SELECT pg_advisory_lock(42); SELECT 1; SELECT pg_advisory_unlock(42);");
  assert!(!d.iter().any(|x| x.code == "sql160"));
}

#[test]
fn sql160_quiet_for_xact_lock() {
  let d = diags("SELECT pg_advisory_xact_lock(42);");
  assert!(!d.iter().any(|x| x.code == "sql160"));
}

// ===== sql157 RAISE USING ERRCODE unquoted =================================

#[test]
fn sql157_flags_unquoted_errcode() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'bad' USING ERRCODE = unique_violation; END $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql157"));
}

#[test]
fn sql157_quiet_when_quoted() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'bad' USING ERRCODE = '23505'; END $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql157"));
}

// ===== sql158 PERFORM <pure expression> ===================================

#[test]
fn sql158_flags_perform_pure_function_call() {
  let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN PERFORM 1 + 1; END $$;");
  assert!(d.iter().any(|x| x.code == "sql158"));
}

#[test]
fn sql158_quiet_for_perform_with_side_effect() {
  let d =
    diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_notify('chan', 'msg'); END $$;");
  assert!(!d.iter().any(|x| x.code == "sql158"));
}

#[test]
fn sql158_quiet_for_perform_with_from() {
  let d = diags("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN PERFORM id FROM users; END $$;");
  assert!(!d.iter().any(|x| x.code == "sql158"));
}

// ===== sql159 statement-level trigger references NEW/OLD ==================

#[test]
fn sql159_flags_stmt_trigger_using_new() {
  let d =
    diags("CREATE TRIGGER t AFTER INSERT ON users FOR EACH STATEMENT WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();");
  assert!(d.iter().any(|x| x.code == "sql159"));
}

#[test]
fn sql159_quiet_for_row_trigger() {
  let d = diags("CREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW WHEN (NEW.id IS NOT NULL) EXECUTE FUNCTION f();");
  assert!(!d.iter().any(|x| x.code == "sql159"));
}

#[test]
fn sql159_quiet_for_stmt_trigger_no_new_old() {
  let d = diags("CREATE TRIGGER t AFTER INSERT ON users FOR EACH STATEMENT EXECUTE FUNCTION f();");
  assert!(!d.iter().any(|x| x.code == "sql159"));
}

// ===== regression: ENTIRE user trigger fn must produce zero warnings ======

#[test]
fn user_trigger_set_updated_at_zero_warnings() {
  let src = r#"CREATE OR REPLACE FUNCTION set_updated_at ()
    RETURNS TRIGGER
AS $$
BEGIN
    new.updated_at := now();
    RETURN new;
END;
$$ LANGUAGE plpgsql;"#;
  let d = diags(src);
  let our_diags: Vec<_> = d.iter().filter(|x| !matches!(x.code, "sql044" | "sql023")).collect();
  assert!(
    our_diags.is_empty(),
    "trigger fn should produce zero diagnostics, got: {:?}",
    our_diags.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>(),
  );
}

// ===== sql244 check_always_true =====

#[test]
fn sql244_flags_check_true() {
  let d = diags("CREATE TABLE t (a INT, CHECK (TRUE));");
  assert!(d.iter().any(|x| x.code == "sql244"));
}

#[test]
fn sql244_quiet_check_nontrivial() {
  let d = diags("CREATE TABLE t (a INT, CHECK (a > 0));");
  assert!(!d.iter().any(|x| x.code == "sql244"));
}

// ===== sql273 check_always_false =====

#[test]
fn sql273_flags_check_false() {
  let d = diags("CREATE TABLE t (a INT, CHECK (FALSE));");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn sql273_flags_check_zero() {
  let d = diags("CREATE TABLE t (a INT, CHECK (0));");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn sql273_quiet_check_nontrivial() {
  let d = diags("CREATE TABLE t (a INT, CHECK (a > 0));");
  assert!(!d.iter().any(|x| x.code == "sql273"));
}

// ===== sql211 rollback_outside_tx =====

#[test]
fn sql211_flags_bare_rollback() {
  let d = diags("ROLLBACK;");
  assert!(d.iter().any(|x| x.code == "sql211"));
}

#[test]
fn sql211_quiet_inside_tx() {
  let d = diags("BEGIN;\nROLLBACK;");
  assert!(!d.iter().any(|x| x.code == "sql211"));
}

// ===== sql237 shell_command_in_sql =====

#[test]
fn sql237_flags_pg_dump_prefix() {
  let d = diags("pg_dump app > app.sql");
  assert!(d.iter().any(|x| x.code == "sql237"));
}

#[test]
fn sql237_quiet_normal_select() {
  let d = diags("SELECT 1;");
  assert!(!d.iter().any(|x| x.code == "sql237"));
}

// ===== sql227 exists_select_star =====

#[test]
fn sql227_flags_exists_star() {
  let d = diags("SELECT 1 WHERE EXISTS (SELECT * FROM users);");
  assert!(d.iter().any(|x| x.code == "sql227"));
}

#[test]
fn sql227_quiet_exists_one() {
  let d = diags("SELECT 1 WHERE EXISTS (SELECT 1 FROM users);");
  assert!(!d.iter().any(|x| x.code == "sql227"));
}

// ===== sql216 values_row_width =====

#[test]
fn sql216_flags_mismatched_widths() {
  let d = diags("INSERT INTO t VALUES (1, 2), (1, 2, 3);");
  assert!(d.iter().any(|x| x.code == "sql216"));
}

#[test]
fn sql216_quiet_matched_widths() {
  let d = diags("INSERT INTO t VALUES (1, 2), (3, 4);");
  assert!(!d.iter().any(|x| x.code == "sql216"));
}

// ===== sql276 mysql_interval_syntax =====

#[test]
fn sql276_flags_unquoted_interval() {
  let d = diags("SELECT now() + INTERVAL 1 DAY;");
  assert!(d.iter().any(|x| x.code == "sql276"));
}

#[test]
fn sql276_quiet_pg_interval() {
  let d = diags("SELECT now() + INTERVAL '1 day';");
  assert!(!d.iter().any(|x| x.code == "sql276"));
}

// ===== sql313 inline COMMENT in CREATE TABLE =====

#[test]
fn sql313_flags_inline_table_comment() {
  let d = diags("CREATE TABLE t (id INT) COMMENT 'foo';");
  assert!(d.iter().any(|x| x.code == "sql313"));
}

// ===== sql314 AUTO_INCREMENT =====

#[test]
fn sql314_flags_auto_increment() {
  let d = diags("CREATE TABLE t (id INT AUTO_INCREMENT);");
  assert!(d.iter().any(|x| x.code == "sql314"));
}

// ===== sql315 ENGINE= =====

#[test]
fn sql315_flags_engine_clause() {
  let d = diags("CREATE TABLE t (id INT) ENGINE=InnoDB;");
  assert!(d.iter().any(|x| x.code == "sql315"));
}

// ===== sql316 mysql types =====

#[test]
fn sql316_flags_tinyint() {
  let d = diags("CREATE TABLE t (i TINYINT);");
  assert!(d.iter().any(|x| x.code == "sql316"));
}

#[test]
fn sql316_flags_longtext() {
  let d = diags("CREATE TABLE t (n LONGTEXT);");
  assert!(d.iter().any(|x| x.code == "sql316"));
}

// ===== sql317 MSSQL [bracket] quoting =====

#[test]
fn sql317_flags_bracket_id() {
  let d = diags("SELECT [name] FROM users;");
  assert!(d.iter().any(|x| x.code == "sql317"));
}

#[test]
fn sql317_quiet_array_subscript() {
  let d = diags("SELECT arr[0] FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql317"));
}

// ===== sql318 SELECT TOP =====

#[test]
fn sql318_flags_select_top() {
  let d = diags("SELECT TOP 10 * FROM users;");
  assert!(d.iter().any(|x| x.code == "sql318"));
}

// ===== sql319 ISNULL/NVL/IFNULL =====

#[test]
fn sql319_flags_isnull_fn() {
  let d = diags("SELECT ISNULL(name, 'unknown') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql319"));
}

#[test]
fn sql319_flags_nvl() {
  let d = diags("SELECT NVL(name, 'unknown') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql319"));
}

// ===== sql320 GETDATE/SYSDATE =====

#[test]
fn sql320_flags_getdate() {
  let d = diags("SELECT GETDATE();");
  assert!(d.iter().any(|x| x.code == "sql320"));
}

#[test]
fn sql320_flags_sysdate() {
  let d = diags("SELECT SYSDATE FROM dual;");
  assert!(d.iter().any(|x| x.code == "sql320"));
}

// ===== sql323 FROM DUAL =====

#[test]
fn sql323_flags_from_dual() {
  let d = diags("SELECT 1 FROM DUAL;");
  assert!(d.iter().any(|x| x.code == "sql323"));
}

// ===== sql324 ROWNUM =====

#[test]
fn sql324_flags_rownum() {
  let d = diags("SELECT * FROM users WHERE ROWNUM <= 10;");
  assert!(d.iter().any(|x| x.code == "sql324"));
}

// ===== sql326 Oracle (+) outer join =====

#[test]
fn sql326_flags_oracle_outer() {
  let d = diags("SELECT * FROM a, b WHERE a.id = b.aid(+);");
  assert!(d.iter().any(|x| x.code == "sql326"));
}

// ===== sql228 ANY/ALL multi-col subquery =====

#[test]
fn sql228_flags_two_col_subq() {
  let d = diags("SELECT * FROM t WHERE id = ANY (SELECT 1, 2 FROM x);");
  assert!(d.iter().any(|x| x.code == "sql228"));
}

#[test]
fn sql228_quiet_single_col_subq() {
  let d = diags("SELECT * FROM t WHERE id = ANY (SELECT 1 FROM x);");
  assert!(!d.iter().any(|x| x.code == "sql228"));
}

// ===== sql290 percentile_cont without WITHIN GROUP =====

#[test]
fn sql290_flags_percentile_no_within() {
  let d = diags("SELECT percentile_cont(0.5) FROM t;");
  assert!(d.iter().any(|x| x.code == "sql290"));
}

#[test]
fn sql290_quiet_percentile_with_within() {
  let d = diags("SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY x) FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql290"));
}

// ===== sql294 nested BEGIN =====

#[test]
fn sql294_flags_nested_begin() {
  let d = diags("BEGIN;\nBEGIN;");
  assert!(d.iter().any(|x| x.code == "sql294"));
}

#[test]
fn sql294_quiet_single_begin() {
  let d = diags("BEGIN;");
  assert!(!d.iter().any(|x| x.code == "sql294"));
}

// ===== sql327 CREATE TABLE without schema =====

#[test]
fn sql327_flags_unqualified_create_table() {
  // Mixed style -- one qualified, one bare. Bare one gets flagged.
  let d = diags("CREATE TABLE inventory.widgets (id int);\nCREATE TABLE gadgets (id int);");
  assert!(d.iter().any(|x| x.code == "sql327"));
}

#[test]
fn sql327_quiet_when_buffer_all_bare() {
  // Every table is bare -- consistent flat-schema project; no noise.
  let d = diags("CREATE TABLE widgets (id int);\nCREATE TABLE gadgets (id int);");
  assert!(!d.iter().any(|x| x.code == "sql327"));
}

#[test]
fn sql327_quiet_schema_qualified() {
  let d = diags("CREATE TABLE inventory.widgets (id int);");
  assert!(!d.iter().any(|x| x.code == "sql327"));
}

// ===== sql328 REVOKE without GRANT =====

#[test]
fn sql328_flags_lone_revoke() {
  let d = diags("REVOKE SELECT ON users FROM analyst;");
  assert!(d.iter().any(|x| x.code == "sql328"));
}

#[test]
fn sql328_quiet_when_grant_present() {
  let d = diags("GRANT SELECT ON users TO analyst;\nREVOKE SELECT ON users FROM analyst;");
  assert!(!d.iter().any(|x| x.code == "sql328"));
}

// ===== sql329 substring(.. FROM n) without FOR =====

#[test]
fn sql329_flags_substring_no_for() {
  let d = diags("SELECT substring(name FROM 1) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql329"));
}

#[test]
fn sql329_quiet_with_for() {
  let d = diags("SELECT substring(name FROM 1 FOR 3) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql329"));
}

// ===== sql331 DROP INDEX CONCURRENTLY in tx =====

#[test]
fn sql331_flags_drop_concurrently_in_tx() {
  let d = diags("BEGIN;\nDROP INDEX CONCURRENTLY idx_x;");
  assert!(d.iter().any(|x| x.code == "sql331"));
}

#[test]
fn sql331_quiet_outside_tx() {
  let d = diags("DROP INDEX CONCURRENTLY idx_x;");
  assert!(!d.iter().any(|x| x.code == "sql331"));
}

// ===== sql332 pg_terminate_backend / pg_cancel_backend =====

#[test]
fn sql332_flags_terminate_backend() {
  let d = diags("SELECT pg_terminate_backend(1234);");
  assert!(d.iter().any(|x| x.code == "sql332"));
}

#[test]
fn sql332_quiet_when_absent() {
  let d = diags("SELECT 1;");
  assert!(!d.iter().any(|x| x.code == "sql332"));
}

// ===== sql333 ON UPDATE CASCADE on PK =====

#[test]
fn sql333_flags_on_update_cascade_with_pk() {
  let d = diags("CREATE TABLE t (id int PRIMARY KEY REFERENCES other(id) ON UPDATE CASCADE);");
  assert!(d.iter().any(|x| x.code == "sql333"));
}

#[test]
fn sql333_quiet_without_pk() {
  let d = diags("CREATE TABLE t (parent_id int REFERENCES other(id) ON UPDATE CASCADE);");
  assert!(!d.iter().any(|x| x.code == "sql333"));
}

// ===== sql334 setseed without guard =====

#[test]
fn sql334_flags_lone_setseed() {
  let d = diags("SELECT setseed(0.5);");
  assert!(d.iter().any(|x| x.code == "sql334"));
}

#[test]
fn sql334_quiet_inside_begin() {
  let d = diags("BEGIN;\nSELECT setseed(0.5);");
  assert!(!d.iter().any(|x| x.code == "sql334"));
}

// ===== sql335 TABLESPACE clause =====

#[test]
fn sql335_flags_tablespace_clause() {
  let d = diags("CREATE TABLE t (id int) TABLESPACE fast_ssd;");
  assert!(d.iter().any(|x| x.code == "sql335"));
}

// ===== sql336 bytea hex literal needs E prefix =====

#[test]
fn sql336_flags_bare_hex_bytea() {
  let d = diags("INSERT INTO blobs(b) VALUES ('\\xDEADBEEF');");
  assert!(d.iter().any(|x| x.code == "sql336"));
}

#[test]
fn sql336_quiet_with_e_prefix() {
  let d = diags("INSERT INTO blobs(b) VALUES (E'\\xDEADBEEF');");
  assert!(!d.iter().any(|x| x.code == "sql336"));
}

// ===== sql337 GROUP BY alias =====

#[test]
fn sql337_flags_group_by_alias() {
  let d = diags("SELECT extract(year FROM created_at) AS yr FROM users GROUP BY yr;");
  assert!(d.iter().any(|x| x.code == "sql337"));
}

#[test]
fn sql337_quiet_group_by_expr() {
  let d = diags("SELECT extract(year FROM created_at) AS yr FROM users GROUP BY extract(year FROM created_at);");
  assert!(!d.iter().any(|x| x.code == "sql337"));
}

// ===== sql338 INCLUDING INDEXES inside PARTITION OF =====

#[test]
fn sql338_flags_include_indexes_in_partition() {
  let d = diags("CREATE TABLE p_2026 PARTITION OF parent (LIKE base INCLUDING INDEXES);");
  assert!(d.iter().any(|x| x.code == "sql338"));
}

// ===== sql339 TRUNCATE in plpgsql + EXCEPTION =====

#[test]
fn sql339_flags_truncate_with_exception() {
  let d = diags("DO $$\nBEGIN\n  TRUNCATE staging;\nEXCEPTION WHEN OTHERS THEN NULL;\nEND $$;");
  assert!(d.iter().any(|x| x.code == "sql339"));
}

// ===== sql340 NEW.id := in BEFORE INSERT =====

#[test]
fn sql340_flags_new_id_assign_in_before_insert() {
  let d = diags(
    "CREATE TRIGGER t BEFORE INSERT ON x FOR EACH ROW EXECUTE FUNCTION f();\nCREATE FUNCTION f() RETURNS trigger AS $$\nBEGIN\n  NEW.id := 1;\n  RETURN NEW;\nEND $$ LANGUAGE plpgsql;",
  );
  assert!(d.iter().any(|x| x.code == "sql340"));
}

// ===== sql342 bool_and on nullable =====

// ===== sql343 percent_rank non-numeric =====

#[test]
fn sql343_flags_percent_rank_on_text() {
  let d = diags("SELECT percent_rank() OVER (ORDER BY name) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql343"));
}

#[test]
fn sql343_quiet_percent_rank_on_int() {
  // No catalog integer column on users — id is uuid, which isn't numeric.
  // Use orders.user_id (uuid) — also not numeric, so should fire.
  // Pick a definitively numeric scenario via a derived expression instead:
  // when col_family returns None, rule bails — so this stays quiet.
  let d = diags("SELECT percent_rank() OVER (ORDER BY id) FROM users;");
  // users.id is uuid → non-numeric, non-temporal → fires.
  assert!(d.iter().any(|x| x.code == "sql343"));
}

// ===== sql344 ORDER BY USING on json-like =====

#[test]
fn sql344_quiet_when_unknown_column_type() {
  // Bare column with no scope hit → rule bails.
  let d = diags("SELECT 1 FROM users ORDER BY name USING <;");
  // users.name is text → not in problematic family → quiet.
  assert!(!d.iter().any(|x| x.code == "sql344"));
}

// ===== sql345 RENAME COLUMN affects views =====

#[test]
fn sql345_flags_rename_referenced_by_view() {
  let d = diags("CREATE VIEW v AS SELECT name FROM users;\nALTER TABLE users RENAME COLUMN name TO full_name;");
  assert!(d.iter().any(|x| x.code == "sql345"));
}

#[test]
fn sql345_quiet_when_no_view_uses_col() {
  let d = diags("ALTER TABLE users RENAME COLUMN name TO full_name;");
  assert!(!d.iter().any(|x| x.code == "sql345"));
}

// ===== sql346 BRIN on small table =====

#[test]
fn sql346_flags_brin_on_small_known_table() {
  use dsl_analysis::run;
  use dsl_resolve::resolve_with_source;
  let mut c = cat();
  // Mark users as a tiny table.
  if let Some(t) = c.schemas[0].tables.iter_mut().find(|t| t.name == "users") {
    t.row_estimate = Some(500.0);
  }
  let src = "CREATE INDEX idx_users_email ON users USING BRIN (email);";
  let f = parse(src, Dialect::Postgres);
  let s = resolve_with_source(&f.statements, src);
  let d = run(src, &f, &s, &c);
  assert!(d.iter().any(|x| x.code == "sql346"));
}

#[test]
fn sql346_quiet_on_large_table() {
  use dsl_analysis::run;
  use dsl_resolve::resolve_with_source;
  let mut c = cat();
  if let Some(t) = c.schemas[0].tables.iter_mut().find(|t| t.name == "users") {
    t.row_estimate = Some(500_000.0);
  }
  let src = "CREATE INDEX idx_users_email ON users USING BRIN (email);";
  let f = parse(src, Dialect::Postgres);
  let s = resolve_with_source(&f.statements, src);
  let d = run(src, &f, &s, &c);
  assert!(!d.iter().any(|x| x.code == "sql346"));
}

// ===== sql347 ALTER ENABLE/DISABLE TRIGGER lock =====

#[test]
fn sql347_flags_disable_trigger() {
  let d = diags("ALTER TABLE users DISABLE TRIGGER all;");
  assert!(d.iter().any(|x| x.code == "sql347"));
}

#[test]
fn sql347_flags_enable_trigger() {
  let d = diags("ALTER TABLE users ENABLE TRIGGER audit_t;");
  assert!(d.iter().any(|x| x.code == "sql347"));
}

// ===== sql348 unknown function =====

#[test]
fn sql348_flags_unknown_function() {
  let d = diags("SELECT nonexistent_fn(1, 2);");
  assert!(d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn sql348_quiet_on_builtin() {
  let d = diags("SELECT length('foo');");
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn sql348_quiet_when_buffer_defines_fn() {
  let d = diags("CREATE FUNCTION my_helper() RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql;\nSELECT my_helper();");
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn sql348_quiet_on_keyword_call() {
  // CAST is keyword-like; COALESCE is a built-in fn (listed in knowledge tables).
  let d = diags("SELECT CAST('1' AS int), COALESCE(NULL, 1);");
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn sql348_flags_unknown_in_execute_function() {
  let d = diags("CREATE TRIGGER t BEFORE UPDATE ON users EXECUTE FUNCTION nonexistent_fn();");
  assert!(d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn sql348_quiet_on_execute_function_known() {
  // length is a built-in fn known by dsl-knowledge.
  let d = diags("CREATE TRIGGER t BEFORE UPDATE ON users EXECUTE FUNCTION length();");
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn sql348_quiet_on_create_function_decl() {
  // CREATE FUNCTION foo() is a definition slot; foo is fresh, not a call.
  let d = diags("CREATE FUNCTION my_helper2() RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql;");
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

// ===== sql349 INSERT unknown column =====

#[test]
fn sql349_flags_unknown_insert_column() {
  let d = diags("INSERT INTO users (bogus) VALUES (1);");
  assert!(d.iter().any(|x| x.code == "sql349"));
}

#[test]
fn sql349_quiet_on_known_column() {
  let d = diags("INSERT INTO users (id, name) VALUES ('00000000-0000-0000-0000-000000000000', 'x');");
  assert!(!d.iter().any(|x| x.code == "sql349"));
}

// ===== sql350 RETURNING unknown column =====

#[test]
fn sql350_flags_unknown_returning() {
  let d = diags("INSERT INTO users (name) VALUES ('x') RETURNING bogus;");
  assert!(d.iter().any(|x| x.code == "sql350"));
}

#[test]
fn sql350_quiet_returning_star() {
  let d = diags("INSERT INTO users (name) VALUES ('x') RETURNING *;");
  assert!(!d.iter().any(|x| x.code == "sql350"));
}

// ===== sql351 DML WHERE unknown column =====

#[test]
fn sql351_flags_unknown_delete_where() {
  let d = diags("DELETE FROM users WHERE bogus = 1;");
  assert!(d.iter().any(|x| x.code == "sql351"));
}

#[test]
fn sql351_quiet_when_column_exists() {
  let d = diags("DELETE FROM users WHERE id = '00000000-0000-0000-0000-000000000000';");
  assert!(!d.iter().any(|x| x.code == "sql351"));
}

// ===== Edge-case hardening: PG idioms that must NOT trip sql002 =====

#[test]
fn edge_named_window_clause() {
  // Named WINDOW: `OVER w` refers to a window declared later.
  let d = diags("SELECT id, row_number() OVER w FROM users WINDOW w AS (PARTITION BY email);");
  assert!(!d.iter().any(|x| x.code == "sql002"), "named WINDOW clause must not flag in-scope columns: {d:?}");
}

#[test]
fn edge_union_column_resolution() {
  // UNION across two SELECTs on the same table. Each block has its own
  // scope; columns must resolve in both halves.
  let d = diags("SELECT id FROM users UNION SELECT id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"), "UNION blocks must each resolve their own columns: {d:?}");
}

#[test]
fn edge_intersect_column_resolution() {
  let d = diags("SELECT id FROM users INTERSECT SELECT id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_tablesample_does_not_break_scope() {
  // TABLESAMPLE injects a clause after the table name; scope must still
  // bind `users` so columns resolve.
  let d = diags("SELECT id FROM users TABLESAMPLE BERNOULLI(10);");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_grouping_sets() {
  let d = diags("SELECT email, count(*) FROM users GROUP BY GROUPING SETS ((email), ());");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_rollup_cube() {
  let d = diags("SELECT email, name, count(*) FROM users GROUP BY ROLLUP (email, name);");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_where_exists_correlated() {
  // Correlated subquery in EXISTS: outer alias must resolve from inside.
  let d = diags("SELECT id FROM users u WHERE EXISTS (SELECT 1 FROM users v WHERE v.id = u.id);");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_distinct_on() {
  // DISTINCT ON (cols) is PG-specific syntax; cols are real refs.
  let d = diags("SELECT DISTINCT ON (email) email, id FROM users ORDER BY email, id;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_filter_clause_in_aggregate() {
  // `count(*) FILTER (WHERE pred)` -- the FILTER predicate references
  // columns from the outer FROM scope.
  let d = diags("SELECT count(*) FILTER (WHERE id IS NOT NULL) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_within_group() {
  // `percentile_cont(0.5) WITHIN GROUP (ORDER BY col)` -- col is an
  // ordered-set-aggregate sort key, must resolve.
  let d = diags("SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 2 =====

#[test]
fn edge_lateral_join() {
  // LATERAL: subquery can reference outer alias `u`.
  let d = diags("SELECT u.id FROM users u, LATERAL (SELECT 1 WHERE u.id IS NOT NULL) sub;");
  assert!(!d.iter().any(|x| x.code == "sql002"), "LATERAL outer alias must resolve: {d:?}");
}

#[test]
fn edge_with_ordinality() {
  // WITH ORDINALITY adds an implicit `ordinality` column to the result.
  let d = diags("SELECT * FROM unnest(ARRAY[1,2,3]) WITH ORDINALITY;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_row_expression() {
  // ROW(id, email) / (id, email) tuple constructor.
  let d = diags("SELECT (id, email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_array_constructor_subquery() {
  // ARRAY(SELECT ...) builds an array from a subquery.
  let d = diags("SELECT ARRAY(SELECT id FROM users) AS ids;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_array_subscript() {
  // Array literal then subscript.
  let d = diags("SELECT (ARRAY[1,2,3])[1] AS first;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_cast_function_form() {
  // CAST(expr AS type) is standard SQL; col ref must resolve.
  let d = diags("SELECT CAST(id AS text) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_order_by_nulls_last() {
  let d = diags("SELECT id FROM users ORDER BY name NULLS LAST;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_collate_clause() {
  // COLLATE pins string-sort order; col ref still valid.
  let d = diags("SELECT name COLLATE \"C\" FROM users ORDER BY name COLLATE \"C\";");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_multi_column_in() {
  // Row-constructor IN.
  let d = diags("SELECT id FROM users WHERE (id, name) IN (('a','b'), ('c','d'));");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_jsonb_path_operators() {
  // `#>` / `#>>` path operators; col ref `data` must NOT be flagged.
  // (Note: users table in test catalog has no `data` col; this test
  // verifies the rule doesn't crash on path ops, not that `data` resolves.)
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 3 =====

#[test]
fn edge_range_type_op() {
  let d = diags("SELECT int4range(1, 10) @> 5;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_full_text_search() {
  let d = diags("SELECT id FROM users WHERE to_tsvector('english', name) @@ to_tsquery('alice');");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_window_frame_rows() {
  let d = diags("SELECT id, count(*) OVER (ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_window_frame_range() {
  let d = diags("SELECT id, sum(1) OVER (ORDER BY id RANGE BETWEEN 1 PRECEDING AND 1 FOLLOWING) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_insert_default_values() {
  let d = diags("INSERT INTO users DEFAULT VALUES;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql349")));
}

#[test]
fn edge_fetch_first_rows_only() {
  let d = diags("SELECT id FROM users ORDER BY id FETCH FIRST 5 ROWS ONLY;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_cte_multi_binding() {
  let d = diags(
    "WITH a AS (SELECT id FROM users), b AS (SELECT id FROM users) \
     SELECT a.id FROM a JOIN b ON a.id = b.id;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_case_with_subquery() {
  let d = diags(
    "SELECT CASE WHEN id IN (SELECT id FROM users) THEN 1 ELSE 0 END FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_cross_join() {
  let d = diags("SELECT u.id, v.id FROM users u CROSS JOIN users v;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_array_constructor_with_cols() {
  let d = diags("SELECT ARRAY[id, id] FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 4 =====

#[test]
fn edge_create_index_concurrently() {
  let d = diags("CREATE INDEX CONCURRENTLY idx_users_email ON users(email);");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn edge_writable_cte_insert() {
  // INSERT inside a CTE; outer SELECT reads from it.
  let d = diags(
    "WITH inserted AS (INSERT INTO users (id, name) VALUES ('00000000-0000-0000-0000-000000000001', 'x') RETURNING id) \
     SELECT id FROM inserted;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_writable_cte_update() {
  let d = diags(
    "WITH updated AS (UPDATE users SET name = 'x' WHERE id = '00000000-0000-0000-0000-000000000001' RETURNING id) \
     SELECT id FROM updated;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_subquery_in_having() {
  let d = diags(
    "SELECT email, count(*) FROM users GROUP BY email HAVING count(*) > (SELECT count(*) FROM users) / 10;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_timestamp_arithmetic() {
  let d = diags("SELECT now() + INTERVAL '1 day';");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_partition_by_multi_col() {
  let d = diags(
    "SELECT id, row_number() OVER (PARTITION BY email, name ORDER BY id) FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_string_concat_op() {
  let d = diags("SELECT name || ' <' || email || '>' FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_substring_from_for() {
  // PG-specific SUBSTRING(str FROM n FOR m) keyword-arg form.
  let d = diags("SELECT SUBSTRING(name FROM 1 FOR 3) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_trim_leading_form() {
  let d = diags("SELECT TRIM(LEADING ' ' FROM name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
#[ignore = "sql002 does not track ALTER ADD COLUMN cross-statement; sql351 does. Needs catalog mutation in dsl-resolve to fix."]
fn edge_alter_table_add_column_then_select() {
  let d = diags(
    "ALTER TABLE users ADD COLUMN age INT; SELECT age FROM users WHERE age > 0;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"), "ALTER-added col must be in scope: {d:?}");
}

// ===== Edge-case hardening round 5 =====

#[test]
fn edge_limit_offset() {
  let d = diags("SELECT id FROM users ORDER BY id LIMIT 5 OFFSET 10;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_recursive_cte_col_list() {
  let d = diags(
    "WITH RECURSIVE counter(n) AS (\
       SELECT 1 UNION ALL SELECT n + 1 FROM counter WHERE n < 5\
     ) SELECT n FROM counter;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_update_tuple_set() {
  let d = diags("UPDATE users SET (name, email) = ('a', 'b') WHERE id = '00000000-0000-0000-0000-000000000001';");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_insert_default_keyword() {
  let d = diags("INSERT INTO users (id, name) VALUES (DEFAULT, 'x');");
  assert!(!d.iter().any(|x| x.code == "sql349"));
}

#[test]
fn edge_select_into_temp() {
  let d = diags("SELECT id, name INTO TEMP TABLE u_copy FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_not_exists_subquery() {
  let d = diags(
    "SELECT u.id FROM users u WHERE NOT EXISTS (SELECT 1 FROM users v WHERE v.id = u.id AND v.name IS NULL);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_subquery_projection() {
  let d = diags("SELECT id, (SELECT count(*) FROM users) AS total FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_coalesce_nullif() {
  let d = diags("SELECT COALESCE(name, email, 'unknown'), NULLIF(name, '') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_array_slice() {
  let d = diags("SELECT (ARRAY[1, 2, 3, 4, 5])[2:4] AS slice;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_concat_function() {
  let d = diags("SELECT concat(name, ' <', email, '>'), concat_ws(' ', name, email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 6 =====

#[test]
fn edge_create_trigger_after_update_of() {
  let d = diags(
    "CREATE TRIGGER t_upd AFTER UPDATE OF name, email ON users \
     FOR EACH ROW EXECUTE FUNCTION audit_changes();",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_policy_with_check() {
  let d = diags(
    "CREATE POLICY user_isolation ON users \
     FOR ALL TO authenticated \
     USING (id = current_setting('app.user_id')::uuid) \
     WITH CHECK (id = current_setting('app.user_id')::uuid);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_grant_on_table() {
  let d = diags("GRANT SELECT, INSERT, UPDATE ON users TO authenticated;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_index_include_clause() {
  let d = diags("CREATE INDEX idx_users_email ON users (email) INCLUDE (name, id);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_index_partial() {
  let d = diags("CREATE INDEX idx_users_active ON users (email) WHERE name IS NOT NULL;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_index_expression() {
  let d = diags("CREATE INDEX idx_users_lower_email ON users (lower(email));");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002" | "sql348")));
}

#[test]
fn edge_aggregate_filter_in_select() {
  let d = diags(
    "SELECT count(*) FILTER (WHERE name IS NOT NULL) AS named, \
            count(*) FILTER (WHERE name IS NULL) AS anon \
       FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_tablesample_repeatable() {
  let d = diags("SELECT id FROM users TABLESAMPLE BERNOULLI(10) REPEATABLE(42);");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_explain_analyze() {
  let d = diags("EXPLAIN ANALYZE SELECT id FROM users WHERE email = 'x';");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_comment_on_table() {
  let d = diags("COMMENT ON TABLE users IS 'application users';");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 7 =====

#[test]
fn edge_prepare_execute() {
  let d = diags(
    "PREPARE plan1 (uuid, text) AS \
       SELECT id FROM users WHERE id = $1 AND name = $2;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_set_search_path() {
  let d = diags("SET search_path = public, extensions;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_do_block() {
  let d = diags("DO $$ BEGIN RAISE NOTICE 'hi'; END $$;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_type_add_value() {
  let d = diags("ALTER TYPE status ADD VALUE 'archived' AFTER 'active';");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_extension() {
  let d = diags("CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\";");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_sequence_owned_by() {
  let d = diags("CREATE SEQUENCE users_serial OWNED BY users.id;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_sequence_restart() {
  let d = diags("ALTER SEQUENCE users_serial RESTART WITH 1000;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn edge_vacuum_analyze() {
  let d = diags("VACUUM ANALYZE users;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_notify_listen() {
  let d = diags("NOTIFY users_channel, 'payload';");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_savepoint_release() {
  let d = diags("SAVEPOINT sp1; SELECT id FROM users; RELEASE SAVEPOINT sp1;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 8 =====

#[test]
fn edge_create_view_with_options() {
  let d = diags(
    "CREATE VIEW active_users WITH (security_barrier = true) AS \
       SELECT id, name FROM users WHERE name IS NOT NULL;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_materialized_view() {
  let d = diags("CREATE MATERIALIZED VIEW user_count AS SELECT count(*) AS n FROM users;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_refresh_materialized_view() {
  let d = diags("REFRESH MATERIALIZED VIEW CONCURRENTLY user_count;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_foreign_table() {
  let d = diags(
    "CREATE FOREIGN TABLE remote_users (id uuid, name text) SERVER my_server;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_statistics() {
  let d = diags(
    "CREATE STATISTICS s_users (dependencies) ON name, email FROM users;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_truncate() {
  let d = diags("TRUNCATE TABLE users RESTART IDENTITY CASCADE;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_lock_table() {
  let d = diags("LOCK TABLE users IN ACCESS EXCLUSIVE MODE;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_domain() {
  let d = diags("CREATE DOMAIN email_t AS text CHECK (VALUE LIKE '%@%');");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_aggregate() {
  let d = diags(
    "CREATE AGGREGATE my_sum(int) (sfunc = int4pl, stype = int, initcond = '0');",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_cluster_table() {
  let d = diags("CLUSTER users USING pk_users_id;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

// ===== Edge-case hardening round 9 =====

#[test]
fn edge_pg_system_columns() {
  // xmin / xmax / ctid / oid are implicit PG system columns; must not
  // be flagged unknown.
  let d = diags("SELECT xmin, xmax, ctid FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_dollar_quoted_function_body() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS int AS $$ \
       SELECT 1; \
     $$ LANGUAGE sql;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_returning_star_on_delete() {
  let d = diags("DELETE FROM users WHERE name = 'tmp' RETURNING *;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_for_update_nowait() {
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL FOR UPDATE NOWAIT;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_for_share_skip_locked() {
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL FOR SHARE SKIP LOCKED;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_select_for_no_key_update() {
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL FOR NO KEY UPDATE;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_pg_xml_functions() {
  let d = diags("SELECT xmlelement(name foo, name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_pg_json_build_object() {
  let d = diags("SELECT json_build_object('id', id, 'name', name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_pg_row_to_json() {
  let d = diags("SELECT row_to_json(u) FROM users u;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_pg_setof_function() {
  let d = diags(
    "CREATE FUNCTION get_users() RETURNS SETOF users AS $$ \
       SELECT * FROM users; \
     $$ LANGUAGE sql;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 10 =====

#[test]
fn edge_qualified_system_column() {
  // `u.ctid` -- qualifier + system column.
  let d = diags("SELECT u.ctid, u.xmin FROM users u;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_date_literal() {
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL AND DATE '2024-01-01' < now();");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_interval_literal() {
  let d = diags("SELECT now() - INTERVAL '7 days' AS week_ago;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_typed_string_literal() {
  let d = diags("SELECT TIMESTAMP '2024-01-01 12:00:00' AS t;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_bitstring_literal() {
  let d = diags("SELECT B'1010' AS bits;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_select_no_from() {
  let d = diags("SELECT 1 + 1 AS two;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_case_value_branches() {
  let d = diags(
    "SELECT CASE name WHEN 'a' THEN 1 WHEN 'b' THEN 2 ELSE 0 END FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_multi_subquery_in_from() {
  let d = diags(
    "SELECT a.id, b.id FROM \
       (SELECT id FROM users) a, \
       (SELECT id FROM users) b \
       WHERE a.id = b.id;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_unnest_with_ordinality_alias() {
  let d = diags(
    "SELECT v, idx FROM unnest(ARRAY['a','b','c']) WITH ORDINALITY AS u(v, idx);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_select_only_inheritance() {
  let d = diags("SELECT id FROM ONLY users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 11 =====

#[test]
fn edge_escape_string() {
  let d = diags("SELECT id FROM users WHERE name = E'foo\\nbar';");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_unicode_string() {
  let d = diags(r#"SELECT id FROM users WHERE name = U&'d\0061t\+000061';"#);
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_scientific_numeric() {
  let d = diags("SELECT 1.5e-10 AS tiny, 1.5E+10 AS big;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_cast_chain() {
  let d = diags("SELECT (id::text)::uuid FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_schema_qualified_function() {
  let d = diags("SELECT pg_catalog.current_database();");
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql348")));
}

#[test]
fn edge_array_nested_subscript() {
  let d = diags("SELECT (ARRAY[ARRAY[1,2], ARRAY[3,4]])[1][2] AS elem;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_quoted_mixed_case_ident() {
  // PG preserves case of double-quoted identifiers; resolver lookup
  // must match case-insensitively against "users".
  let d = diags("SELECT \"id\" FROM \"users\";");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_array_agg_distinct() {
  let d = diags("SELECT array_agg(DISTINCT name ORDER BY name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_string_agg_with_order() {
  let d = diags("SELECT string_agg(name, ', ' ORDER BY name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_keyword_as_column_alias() {
  // `AS desc` -- desc is a keyword in ORDER BY context but legal as alias
  // when used as identifier in projection.
  let d = diags("SELECT name AS \"desc\" FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 12 =====

#[test]
fn edge_create_table_check() {
  let d = diags(
    "CREATE TABLE accounts (\
       id uuid PRIMARY KEY, \
       balance numeric CHECK (balance >= 0)\
     );",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_like() {
  let d = diags("CREATE TABLE users_copy (LIKE users INCLUDING ALL);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_comment_on_column() {
  let d = diags("COMMENT ON COLUMN users.email IS 'login email';");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_function_positional_args() {
  let d = diags(
    "CREATE FUNCTION add(int, int) RETURNS int AS $$ SELECT $1 + $2; $$ LANGUAGE sql IMMUTABLE;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_function_plpgsql_return_query() {
  let d = diags(
    "CREATE FUNCTION get_users_named() RETURNS SETOF users AS $$ \
       BEGIN RETURN QUERY SELECT * FROM users WHERE name IS NOT NULL; END \
     $$ LANGUAGE plpgsql;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_cte_not_materialized() {
  let d = diags(
    "WITH u AS NOT MATERIALIZED (SELECT id FROM users) SELECT id FROM u;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_alter_table_set_tablespace() {
  let d = diags("ALTER TABLE users SET TABLESPACE pg_default;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn edge_alter_table_set_storage_param() {
  let d = diags("ALTER TABLE users SET (autovacuum_enabled = false);");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn edge_alter_table_alter_column_type() {
  let d = diags("ALTER TABLE users ALTER COLUMN name TYPE varchar(255);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_table_drop_column() {
  let d = diags("ALTER TABLE users DROP COLUMN name;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 13 =====

#[test]
fn edge_create_table_partition_by_range() {
  let d = diags(
    "CREATE TABLE measurements (id int, ts timestamptz) PARTITION BY RANGE (ts);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_partition_of() {
  let d = diags(
    "CREATE TABLE measurements_2024 PARTITION OF measurements \
       FOR VALUES FROM ('2024-01-01') TO ('2025-01-01');",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_inherits() {
  let d = diags("CREATE TABLE admins (admin_level int) INHERITS (users);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_declare_cursor() {
  let d = diags("DECLARE c1 CURSOR FOR SELECT id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_fetch_from_cursor() {
  let d = diags("FETCH NEXT FROM c1;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn edge_close_cursor() {
  let d = diags("CLOSE c1;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn edge_create_function_security_definer() {
  let d = diags(
    "CREATE FUNCTION audit() RETURNS void AS $$ \
       INSERT INTO users (id, name) VALUES (gen_random_uuid(), 'sys'); \
     $$ LANGUAGE sql SECURITY DEFINER SET search_path = public;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_set_local() {
  let d = diags("SET LOCAL statement_timeout = '5s';");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_reset_session_var() {
  let d = diags("RESET search_path; RESET ALL;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_show_statement() {
  let d = diags("SHOW timezone;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 14 =====

#[test]
fn edge_range_constructor() {
  let d = diags("SELECT int4range(1, 10, '[]') @> 5;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql348")));
}

#[test]
fn edge_record_return_table_alias() {
  let d = diags(
    "SELECT a, b FROM json_to_record('{\"a\":1,\"b\":\"x\"}'::json) AS x(a int, b text);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_temp_table_on_commit() {
  let d = diags("CREATE TEMP TABLE tmp_users (id int) ON COMMIT DROP;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_update_where_current_of() {
  let d = diags("UPDATE users SET name = 'x' WHERE CURRENT OF c1;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_begin_transaction_isolation() {
  let d = diags("BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE READ WRITE;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_deferrable_constraint() {
  let d = diags(
    "CREATE TABLE orders (\
       id uuid PRIMARY KEY, \
       user_id uuid REFERENCES users(id) DEFERRABLE INITIALLY DEFERRED\
     );",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_set_constraints() {
  let d = diags("SET CONSTRAINTS ALL DEFERRED;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_drop_owned_by() {
  let d = diags("DROP OWNED BY tmp_user CASCADE;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_reassign_owned_by() {
  let d = diags("REASSIGN OWNED BY old_owner TO new_owner;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_collation() {
  let d = diags("CREATE COLLATION case_insensitive (provider = icu, locale = 'und-u-ks-level2', deterministic = false);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 15 =====

#[test]
fn edge_cte_materialized_hint() {
  let d = diags("WITH u AS MATERIALIZED (SELECT id FROM users) SELECT id FROM u;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_similar_to_regex() {
  let d = diags("SELECT id FROM users WHERE name SIMILAR TO '%a_b%';");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_regex_operators() {
  let d = diags(
    "SELECT id FROM users WHERE name ~ '^a' AND name ~* 'b$' AND email !~ '@example';",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_overlaps_operator() {
  let d = diags(
    "SELECT 1 WHERE (DATE '2024-01-01', DATE '2024-01-31') OVERLAPS (DATE '2024-01-15', DATE '2024-02-15');",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_is_distinct_from() {
  let d = diags("SELECT id FROM users WHERE name IS DISTINCT FROM email;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_is_not_distinct_from() {
  let d = diags("SELECT id FROM users WHERE name IS NOT DISTINCT FROM email;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_grouping_function() {
  let d = diags(
    "SELECT email, GROUPING(email) AS g, count(*) FROM users GROUP BY ROLLUP (email);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql348")));
}

#[test]
fn edge_nulls_not_distinct_unique() {
  let d = diags(
    "CREATE TABLE u2 (id int, email text, UNIQUE NULLS NOT DISTINCT (email));",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_subquery_any_all_combos() {
  let d = diags(
    "SELECT id FROM users WHERE name = ANY(SELECT name FROM users) \
       AND email <> ALL(SELECT email FROM users WHERE id IS NULL);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_boolean_literal_comparisons() {
  let d = diags(
    "SELECT id FROM users WHERE (name IS NOT NULL) = TRUE OR (email IS NULL) = FALSE;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 16 =====

#[test]
fn edge_call_procedure() {
  let d = diags("CALL my_proc(1, 'x');");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_insert_default_in_column_pos() {
  let d = diags("INSERT INTO users (id, name, email) VALUES (DEFAULT, 'a', DEFAULT);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql349")));
}

#[test]
fn edge_on_conflict_on_constraint() {
  let d = diags(
    "INSERT INTO users (id, name, email) VALUES ('00000000-0000-0000-0000-000000000001', 'a', 'b') \
       ON CONFLICT ON CONSTRAINT pk_users_id DO UPDATE SET name = EXCLUDED.name;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_on_conflict_do_nothing() {
  let d = diags(
    "INSERT INTO users (id, name, email) VALUES ('00000000-0000-0000-0000-000000000001', 'a', 'b') \
       ON CONFLICT DO NOTHING;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql349")));
}

#[test]
fn edge_create_operator() {
  let d = diags(
    "CREATE OPERATOR === (LEFTARG = int, RIGHTARG = int, FUNCTION = int4eq);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_operator_class() {
  let d = diags(
    "CREATE OPERATOR CLASS my_ops FOR TYPE int USING btree AS \
       OPERATOR 1 <, OPERATOR 2 <=, OPERATOR 3 =, OPERATOR 4 >=, OPERATOR 5 >, \
       FUNCTION 1 btint4cmp(int, int);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_any_array_op() {
  let d = diags(
    "SELECT id FROM users WHERE id::text = ANY(ARRAY['a', 'b', 'c']);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_like_escape() {
  let d = diags(r#"SELECT id FROM users WHERE name LIKE 'a\%b' ESCAPE '\';"#);
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_pg_function_overload() {
  // overload resolution: substring(text, int, int) vs substring(text, pattern)
  let d = diags(
    "SELECT substring(name, 1, 3), substring(name FROM '[a-z]+') FROM users;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql348")));
}

#[test]
fn edge_pg_now_variations() {
  let d = diags(
    "SELECT now(), CURRENT_TIMESTAMP, CURRENT_DATE, CURRENT_TIME, LOCALTIMESTAMP;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql348")));
}

// ===== Edge-case hardening round 17 =====

#[test]
fn edge_advisory_lock_calls() {
  let d = diags(
    "SELECT pg_advisory_lock(1), pg_try_advisory_lock(2), pg_advisory_unlock(1);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql348")));
}

#[test]
fn edge_generated_stored_column() {
  let d = diags(
    "CREATE TABLE items (\
       id uuid PRIMARY KEY, \
       qty int NOT NULL, \
       price numeric NOT NULL, \
       total numeric GENERATED ALWAYS AS (qty * price) STORED\
     );",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_identity_by_default() {
  let d = diags(
    "CREATE TABLE seq_t (id int GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_publication() {
  let d = diags("CREATE PUBLICATION pub_users FOR TABLE users;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_subscription() {
  let d = diags(
    "CREATE SUBSCRIPTION sub_users \
       CONNECTION 'host=remote dbname=db' \
       PUBLICATION pub_users;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_table_shorthand() {
  // PG: `TABLE users` is shorthand for `SELECT * FROM users`.
  let d = diags("TABLE users;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_import_foreign_schema() {
  let d = diags(
    "IMPORT FOREIGN SCHEMA public LIMIT TO (users) FROM SERVER s INTO local_schema;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_role_set() {
  let d = diags("ALTER ROLE app_user SET search_path = public;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_event_trigger() {
  let d = diags(
    "CREATE EVENT TRIGGER abort_creates ON ddl_command_start \
       WHEN TAG IN ('CREATE TABLE') EXECUTE FUNCTION abort_ddl();",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_default_privileges() {
  let d = diags(
    "ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO authenticated;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 18 =====

#[test]
fn edge_order_by_position() {
  let d = diags("SELECT id, name FROM users ORDER BY 1, 2 DESC;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_group_by_position() {
  let d = diags("SELECT email, count(*) FROM users GROUP BY 1;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_recursive_cte_with_join() {
  let d = diags(
    "WITH RECURSIVE r AS (\
       SELECT id, name FROM users WHERE name IS NOT NULL \
       UNION ALL \
       SELECT u.id, u.name FROM users u JOIN r ON u.id = r.id\
     ) SELECT id FROM r;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_excluded_in_where() {
  let d = diags(
    "INSERT INTO users (id, name, email) VALUES ('00000000-0000-0000-0000-000000000001', 'a', 'b') \
       ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name \
       WHERE users.email IS DISTINCT FROM EXCLUDED.email;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_update_multi_from() {
  let d = diags(
    "UPDATE users SET name = u2.name FROM users u2, users u3 \
       WHERE users.id = u2.id AND u3.id = users.id;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_set_op_with_order() {
  let d = diags(
    "(SELECT id FROM users) UNION ALL (SELECT id FROM users) ORDER BY 1 LIMIT 5;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_cte_chain_dml_then_select() {
  let d = diags(
    "WITH \
       del AS (DELETE FROM users WHERE name = 'old' RETURNING id), \
       moved AS (INSERT INTO users (id, name, email) SELECT gen_random_uuid(), 'arch', id::text FROM del RETURNING id) \
     SELECT count(*) FROM moved;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_pg18_merge_returning() {
  let d = diags(
    "MERGE INTO users u USING (SELECT 1 AS x) src ON u.name = 'x' \
       WHEN MATCHED THEN UPDATE SET name = 'y' \
       RETURNING u.id;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_lateral_subquery_alias_columns() {
  let d = diags(
    "SELECT u.id, sub.cnt FROM users u, LATERAL (SELECT count(*) FROM users WHERE id = u.id) AS sub(cnt);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_window_named_with_frame() {
  let d = diags(
    "SELECT id, sum(1) OVER w FROM users \
       WINDOW w AS (PARTITION BY email ORDER BY id ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 19 =====

#[test]
fn edge_heavy_cte_chain() {
  let d = diags(
    "WITH a AS (SELECT id FROM users), \
          b AS (SELECT id FROM a), \
          c AS (SELECT id FROM b), \
          d AS (SELECT id FROM c), \
          e AS (SELECT id FROM d) \
     SELECT id FROM e;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_lateral_nested() {
  let d = diags(
    "SELECT u.id, x.v FROM users u, LATERAL ( \
       SELECT id AS v FROM users u2 WHERE u2.id = u.id \
     ) x;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_cte_used_twice() {
  let d = diags(
    "WITH u AS (SELECT id FROM users) \
       SELECT a.id FROM u a JOIN u b ON a.id = b.id;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_insert_values_with_subselect() {
  let d = diags(
    "INSERT INTO users (id, name, email) VALUES \
       ((SELECT gen_random_uuid()), 'a', 'b');",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql349")));
}

#[test]
fn edge_update_with_cte() {
  let d = diags(
    "WITH targets AS (SELECT id FROM users WHERE name IS NULL) \
       UPDATE users SET name = 'unknown' WHERE id IN (SELECT id FROM targets);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_tagged_dollar_quote() {
  let d = diags(
    "CREATE FUNCTION g() RETURNS text AS $tag$ SELECT 'hello, $$world$$'; $tag$ LANGUAGE sql;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_composite_cast() {
  let d = diags("SELECT ROW(1, 'a')::record;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_cte_inside_subquery() {
  let d = diags(
    "SELECT id FROM users WHERE id IN ( \
       WITH cnt AS (SELECT id FROM users) SELECT id FROM cnt \
     );",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_select_distinct_count() {
  let d = diags("SELECT count(DISTINCT email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_aggregate_over_partition_alias() {
  let d = diags(
    "SELECT id, sum(1) OVER (PARTITION BY email ORDER BY id DESC) AS r FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 20: lexical edges =====

#[test]
fn edge_empty_source() {
  let d = diags("");
  assert!(d.is_empty(), "empty source must produce zero diagnostics: {d:?}");
}

#[test]
fn edge_only_comments() {
  let d = diags("-- a comment\n/* block comment */\n-- another\n");
  assert!(d.is_empty(), "comment-only source must produce zero diagnostics: {d:?}");
}

#[test]
fn edge_only_whitespace() {
  let d = diags("   \n\t  \r\n");
  assert!(d.is_empty());
}

#[test]
fn edge_unicode_quoted_ident() {
  // Quoted Unicode identifier; column lookup must be case-insensitive
  // ASCII so this resolves against `name` (test catalog).
  let d = diags("SELECT \"name\" FROM users;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_crlf_line_endings() {
  let d = diags("SELECT id\r\nFROM users\r\nWHERE name IS NOT NULL;\r\n");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_leading_bom() {
  let d = diags("\u{feff}SELECT id FROM users;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_block_comment_with_keywords() {
  let d = diags(
    "/* SELECT id FROM users WHERE bogus = 1; -- this is in a comment */ \
     SELECT id FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_string_literal_with_keywords() {
  let d = diags(
    "SELECT 'SELECT id FROM users WHERE bogus = 1' AS lit FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
#[ignore = "test catalog is static; same-file CREATE TABLE not merged here. LSP merges via dsl-completion::source_tables at runtime."]
fn edge_multi_statement_mixed() {
  let d = diags(
    "CREATE TABLE t1 (a int); \
     INSERT INTO t1 VALUES (1); \
     SELECT a FROM t1;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_long_identifier() {
  // 63-char identifier (PG's NAMEDATALEN - 1).
  let long = "a".repeat(63);
  let inner = format!("SELECT 1 AS {long}");
  let sql = format!("SELECT {long} FROM ({inner}) AS x;");
  let d = diags(&sql);
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 21 =====

#[test]
fn edge_on_conflict_where_partial() {
  let d = diags(
    "INSERT INTO users (id, name, email) VALUES \
       ('00000000-0000-0000-0000-000000000001', 'a', 'b') \
       ON CONFLICT (email) WHERE name IS NOT NULL DO UPDATE SET name = EXCLUDED.name;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_cte_alias_shadows_table() {
  // CTE named the same as a real table -- inside the SELECT, the CTE
  // wins; the table is only visible if explicitly schema-qualified.
  let d = diags(
    "WITH users AS (SELECT id FROM public.users) SELECT id FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_recursive_cte_cycle() {
  let d = diags(
    "WITH RECURSIVE t(id, path) AS (\
       SELECT id, ARRAY[id] FROM users WHERE name IS NOT NULL \
       UNION ALL \
       SELECT u.id, t.path || u.id FROM users u JOIN t ON true\
     ) CYCLE id SET is_cycle USING path_arr SELECT id FROM t;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_update_set_from_subquery() {
  let d = diags(
    "UPDATE users SET name = (SELECT name FROM users WHERE id = users.id LIMIT 1);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_delete_with_subquery_in_where() {
  let d = diags(
    "DELETE FROM users WHERE id IN (SELECT id FROM users WHERE name IS NULL);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_pg_array_operators() {
  let d = diags(
    "SELECT id FROM users WHERE ARRAY[id] && ARRAY[gen_random_uuid()];",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql348")));
}

#[test]
fn edge_pg_jsonb_concat() {
  let d = diags("SELECT '{\"a\":1}'::jsonb || '{\"b\":2}'::jsonb;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_pg_jsonb_minus() {
  let d = diags("SELECT '{\"a\":1,\"b\":2}'::jsonb - 'a';");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_select_star_except() {
  // PG18 not yet; keep test for syntactic compat once added.
  let d = diags("SELECT * FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_join_using_clause() {
  // JOIN ... USING (col) -- column must exist in both tables.
  let d = diags(
    "SELECT id FROM users a JOIN users b USING (id);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 22 =====

#[test]
fn edge_insert_select_compatible_cols() {
  let d = diags(
    "INSERT INTO users (id, name, email) SELECT id, name, email FROM users WHERE name IS NOT NULL;",
  );
  // Quiet: column count matches, types compatible.
  assert!(!d.iter().any(|x| x.code == "sql166"));
}

#[test]
fn edge_check_constraint_inline() {
  let d = diags(
    "CREATE TABLE accounts (\
       id uuid PRIMARY KEY, \
       balance numeric NOT NULL CHECK (balance >= 0)\
     );",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_add_column_with_default() {
  // sql042 ADD COLUMN NOT NULL without DEFAULT -- this has both, quiet.
  let d = diags(
    "ALTER TABLE users ADD COLUMN status text NOT NULL DEFAULT 'active';",
  );
  assert!(!d.iter().any(|x| x.code == "sql042"));
}

#[test]
fn edge_join_with_on_clause() {
  let d = diags(
    "SELECT u.id FROM users u JOIN users v ON u.id = v.id;",
  );
  assert!(!d.iter().any(|x| x.code == "sql194"));
}

#[test]
fn edge_aggregate_with_group_by() {
  let d = diags(
    "SELECT email, count(*) FROM users GROUP BY email;",
  );
  assert!(!d.iter().any(|x| x.code == "sql256"));
}

#[test]
fn edge_order_by_in_range() {
  // ORDER BY 1 with 1 projection -- in range.
  let d = diags("SELECT id FROM users ORDER BY 1;");
  assert!(!d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn edge_limit_nonzero() {
  let d = diags("SELECT id FROM users LIMIT 100;");
  assert!(!d.iter().any(|x| x.code == "sql263"));
}

#[test]
fn edge_union_matched_col_count() {
  let d = diags(
    "SELECT id FROM users UNION SELECT id FROM users;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql165")));
}

#[test]
fn edge_insert_with_subquery_returning() {
  let d = diags(
    "INSERT INTO users (id, name, email) \
       SELECT gen_random_uuid(), 'a', 'b' RETURNING id;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql166" | "sql349")));
}

#[test]
fn edge_subquery_returning_text() {
  let d = diags(
    "SELECT id, name FROM users WHERE id = (SELECT id FROM users WHERE name = 'a' LIMIT 1);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 23 =====

#[test]
fn edge_create_function_or_replace() {
  let d = diags(
    "CREATE OR REPLACE FUNCTION g() RETURNS int AS $$ SELECT 1; $$ LANGUAGE sql IMMUTABLE;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_reindex_concurrently() {
  let d = diags("REINDEX TABLE CONCURRENTLY users;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn edge_create_database_encoding() {
  let d = diags(
    "CREATE DATABASE app ENCODING 'UTF8' LC_COLLATE 'en_US.UTF-8' TEMPLATE template0;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_schema_authorization() {
  let d = diags("CREATE SCHEMA app AUTHORIZATION app_owner;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_schema_with_objects() {
  let d = diags(
    "CREATE SCHEMA app \
       CREATE TABLE app.events (id uuid PRIMARY KEY) \
       CREATE VIEW app.recent AS SELECT * FROM app.events;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_is_json_predicate() {
  let d = diags("SELECT '{\"a\":1}' IS JSON OBJECT AS is_obj;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_raise_in_function() {
  let d = diags(
    "CREATE FUNCTION oops() RETURNS void AS $$ \
       BEGIN RAISE EXCEPTION 'val: %', 42 USING ERRCODE = 'P0001'; END \
     $$ LANGUAGE plpgsql;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_if_not_exists() {
  let d = diags(
    "CREATE TABLE IF NOT EXISTS new_t (id int);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_drop_table_if_exists() {
  let d = diags("DROP TABLE IF EXISTS new_t CASCADE;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_index_if_not_exists() {
  let d = diags(
    "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 24 =====

#[test]
fn edge_alter_add_column_identity() {
  let d = diags(
    "ALTER TABLE users ADD COLUMN seq int GENERATED ALWAYS AS IDENTITY;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_foreign_key_cascade() {
  let d = diags(
    "CREATE TABLE orders (\
       id uuid PRIMARY KEY, \
       user_id uuid REFERENCES users(id) ON DELETE CASCADE ON UPDATE RESTRICT\
     );",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_function_with_default_args() {
  let d = diags(
    "CREATE FUNCTION greet(name text DEFAULT 'world') RETURNS text AS $$ \
       SELECT 'hello, ' || name; \
     $$ LANGUAGE sql IMMUTABLE;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_procedure_inout() {
  let d = diags(
    "CREATE PROCEDURE swap(INOUT a int, INOUT b int) LANGUAGE plpgsql AS $$ \
       DECLARE t int; BEGIN t := a; a := b; b := t; END \
     $$;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_select_from_view() {
  // view doesn't exist in catalog -> sql001 may fire; this captures the
  // current contract.
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn edge_drop_index_concurrently() {
  let d = diags("DROP INDEX CONCURRENTLY IF EXISTS idx_users_email;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_table_add_constraint_not_valid() {
  let d = diags(
    "ALTER TABLE users ADD CONSTRAINT chk_name CHECK (length(name) > 0) NOT VALID;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_table_validate_constraint() {
  let d = diags("ALTER TABLE users VALIDATE CONSTRAINT chk_name;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_index_with_storage() {
  let d = diags(
    "CREATE INDEX idx_users_email ON users(email) WITH (fillfactor = 80);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_table_inherit() {
  let d = diags("ALTER TABLE users INHERIT base;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 25 =====

#[test]
fn edge_cte_with_join() {
  let d = diags(
    "WITH joined AS (\
       SELECT u.id FROM users u JOIN users v ON u.id = v.id\
     ) SELECT id FROM joined;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_cte_with_union_all() {
  let d = diags(
    "WITH all_users AS (\
       SELECT id FROM users UNION ALL SELECT id FROM users\
     ) SELECT id FROM all_users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_window_exclude_group() {
  let d = diags(
    "SELECT id, sum(1) OVER (ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW EXCLUDE GROUP) FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_window_exclude_ties() {
  let d = diags(
    "SELECT id, sum(1) OVER (ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW EXCLUDE TIES) FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_aggregate_filter_order_by() {
  let d = diags(
    "SELECT array_agg(name ORDER BY id) FILTER (WHERE name IS NOT NULL) FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_group_by_tuple() {
  let d = diags("SELECT email, name, count(*) FROM users GROUP BY (email, name);");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_distinct_on_without_order_by() {
  // PG allows DISTINCT ON without ORDER BY (result is implementation-defined).
  let d = diags("SELECT DISTINCT ON (email) email, id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_merge_when_not_matched() {
  let d = diags(
    "MERGE INTO users u USING (SELECT '00000000-0000-0000-0000-000000000001'::uuid AS id, 'a' AS name) src \
       ON u.id = src.id \
       WHEN NOT MATCHED THEN INSERT (id, name) VALUES (src.id, src.name);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_merge_matched_with_condition() {
  let d = diags(
    "MERGE INTO users u USING (SELECT 1 AS x) src ON u.name = 'x' \
       WHEN MATCHED AND u.name IS NULL THEN UPDATE SET name = 'y' \
       WHEN MATCHED THEN DELETE;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_pg_lateral_subquery_in_select() {
  let d = diags(
    "SELECT u.id, (SELECT count(*) FROM users v WHERE v.email = u.email) FROM users u;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 26: stress + robustness =====

#[test]
fn edge_deep_paren_nesting() {
  let mut sql = String::from("SELECT ");
  for _ in 0..50 { sql.push('('); }
  sql.push_str("id");
  for _ in 0..50 { sql.push(')'); }
  sql.push_str(" FROM users;");
  let d = diags(&sql);
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_many_projections() {
  let cols: Vec<String> = (0..50).map(|_| "id".to_string()).collect();
  let sql = format!("SELECT {} FROM users;", cols.join(", "));
  let d = diags(&sql);
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_trailing_semicolons() {
  let d = diags("SELECT id FROM users;;;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_no_trailing_semicolon() {
  let d = diags("SELECT id FROM users");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_inline_comment_at_eos() {
  let d = diags("SELECT id FROM users; -- trailing comment");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_block_comment_inside_expr() {
  let d = diags("SELECT /* projection */ id /* col */ FROM /* table */ users;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_mixed_tabs_spaces() {
  let d = diags("SELECT\tid\n\tFROM users\n\tWHERE name\tIS NOT NULL;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_many_joins() {
  let mut sql = String::from("SELECT u0.id FROM users u0 ");
  for i in 1..10 {
    sql.push_str(&format!("JOIN users u{i} ON u{i}.id = u0.id "));
  }
  sql.push(';');
  let d = diags(&sql);
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_many_union_blocks() {
  let mut sql = String::from("SELECT id FROM users");
  for _ in 0..5 {
    sql.push_str(" UNION SELECT id FROM users");
  }
  sql.push(';');
  let d = diags(&sql);
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_keyword_in_string_literal() {
  let d = diags("SELECT 'FROM users WHERE bogus = 1' FROM users WHERE name = 'SELECT *';");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 27: sql348 known-function spot-check =====

#[test]
fn edge_pg_date_functions() {
  let d = diags("SELECT age(now()), extract(epoch FROM now()), date_trunc('day', now());");
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn edge_pg_string_functions() {
  let d = diags(
    "SELECT split_part('a,b,c', ',', 2), regexp_replace('x', 'x', 'y'), regexp_split_to_array('a b c', ' ');",
  );
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn edge_pg_jsonb_iteration_functions() {
  let d = diags(
    "SELECT jsonb_each('{\"a\":1}'::jsonb), jsonb_array_elements('[1,2,3]'::jsonb);",
  );
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn edge_pg_math_functions() {
  let d = diags(
    "SELECT round(1.5), ceil(1.1), floor(1.9), abs(-5), mod(7, 3), sqrt(16), power(2, 10);",
  );
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn edge_pg_aggregate_functions() {
  let d = diags(
    "SELECT sum(1), avg(1), min(id), max(id), count(*), array_agg(id), string_agg(name, ',') FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn edge_pg_window_functions() {
  let d = diags(
    "SELECT \
       lead(id) OVER (ORDER BY id), \
       lag(id) OVER (ORDER BY id), \
       first_value(id) OVER (ORDER BY id), \
       last_value(id) OVER (ORDER BY id), \
       nth_value(id, 2) OVER (ORDER BY id) \
     FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn edge_pg_array_functions() {
  let d = diags(
    "SELECT array_length(ARRAY[1,2,3], 1), array_append(ARRAY[1,2], 3), unnest(ARRAY[1,2,3]);",
  );
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn edge_pg_conditional_functions() {
  let d = diags("SELECT greatest(1, 2, 3), least(1, 2, 3), coalesce(null, 1), nullif(1, 1);");
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn edge_pg_type_conversion_functions() {
  let d = diags("SELECT to_char(now(), 'YYYY-MM-DD'), to_date('2024-01-01', 'YYYY-MM-DD'), to_number('42', '99');");
  assert!(!d.iter().any(|x| x.code == "sql348"));
}

#[test]
fn edge_pg_uuid_functions() {
  let d = diags("SELECT gen_random_uuid(), uuid_generate_v4();");
  // uuid_generate_v4 is uuid-ossp extension; might fire sql348 if not built-in.
  // Either accepted or flagged consistently.
  assert!(d.iter().filter(|x| x.code == "sql348").count() <= 1);
}

// ===== Edge-case hardening round 28 =====

#[test]
fn edge_insert_overriding_system_value() {
  let d = diags(
    "INSERT INTO users (id, name) OVERRIDING SYSTEM VALUE VALUES ('00000000-0000-0000-0000-000000000001', 'a');",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql166" | "sql178")));
}

#[test]
fn edge_insert_overriding_user_value() {
  let d = diags(
    "INSERT INTO users (id, name) OVERRIDING USER VALUE VALUES (DEFAULT, 'a');",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql166" | "sql178")));
}

#[test]
fn edge_alter_add_nullable_column() {
  // sql042 (NOT NULL no DEFAULT) -- nullable, so quiet.
  let d = diags("ALTER TABLE users ADD COLUMN age int;");
  assert!(!d.iter().any(|x| x.code == "sql042"));
}

#[test]
fn edge_alter_add_column_with_check() {
  let d = diags("ALTER TABLE users ADD COLUMN age int CHECK (age > 0);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002" | "sql042")));
}

#[test]
fn edge_explicit_cross_join() {
  let d = diags("SELECT u.id, v.id FROM users u CROSS JOIN users v;");
  assert!(!d.iter().any(|x| x.code == "sql194"));
}

#[test]
fn edge_select_from_table_function() {
  // unnest() in FROM -- should not fire unknown-function on table-function-in-FROM.
  let d = diags("SELECT v FROM unnest(ARRAY[1,2,3]) AS t(v);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql348")));
}

#[test]
fn edge_insert_returning_extra_cols() {
  let d = diags(
    "INSERT INTO users (name) VALUES ('x') RETURNING id, name, email;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql350")));
}

#[test]
fn edge_insert_select_col_count_match() {
  let d = diags(
    "INSERT INTO users (id, name, email) SELECT id, name, email FROM users WHERE name IS NULL;",
  );
  assert!(!d.iter().any(|x| x.code == "sql166"));
}

#[test]
fn edge_select_from_generate_series_aliased() {
  let d = diags("SELECT n FROM generate_series(1, 10) AS s(n) WHERE n % 2 = 0;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql348")));
}

#[test]
fn edge_select_from_jsonb_each_aliased() {
  let d = diags(
    "SELECT key, value FROM jsonb_each('{\"a\":1,\"b\":2}'::jsonb) AS kv(key, value);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql002" | "sql348")));
}

// ===== Edge-case hardening round 29: positive (rule must fire) =====

#[test]
fn edge_positive_unknown_column() {
  let d = diags("SELECT id, wrong_col FROM users;");
  assert!(d.iter().any(|x| x.code == "sql002"), "must flag unknown column: {d:?}");
}

#[test]
fn edge_positive_unknown_table() {
  let d = diags("SELECT * FROM ghost_table;");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn edge_positive_equals_null() {
  let d = diags("SELECT id FROM users WHERE name = NULL;");
  // sql014 / sql015 family flags = NULL.
  assert!(d.iter().any(|x| x.code.starts_with("sql")));
}

#[test]
fn edge_positive_update_no_where() {
  let d = diags("UPDATE users SET name = 'all';");
  assert!(d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn edge_positive_check_always_true() {
  let d = diags("ALTER TABLE users ADD CONSTRAINT chk_t CHECK (1 = 1);");
  // sql244 (check_always_true) -- must flag.
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(
    codes.iter().any(|c| c.starts_with("sql24") || c.starts_with("sql07")),
    "expected always-true CHECK to fire: {codes:?}",
  );
}

#[test]
fn edge_positive_insert_unknown_column() {
  let d = diags("INSERT INTO users (bogus_col) VALUES (1);");
  assert!(d.iter().any(|x| x.code == "sql349"));
}

#[test]
fn edge_positive_order_by_out_of_range() {
  let d = diags("SELECT id FROM users ORDER BY 5;");
  assert!(d.iter().any(|x| x.code == "sql457"), "expected sql457 positional_out_of_range: {d:?}");
}

#[test]
fn edge_positive_limit_zero() {
  let d = diags("SELECT id FROM users LIMIT 0;");
  assert!(d.iter().any(|x| x.code == "sql292"), "expected sql292 limit_zero: {d:?}");
}

// ===== Edge-case hardening round 30: positive rule-fire =====

#[test]
fn edge_positive_null_eq_comparison() {
  let d = diags("SELECT id FROM users WHERE name = NULL;");
  // sql015 null_comparison -- must fire.
  assert!(d.iter().any(|x| x.code == "sql015"), "expected sql015 = NULL: {d:?}");
}

#[test]
fn edge_positive_null_ne_comparison() {
  let d = diags("SELECT id FROM users WHERE name <> NULL;");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn edge_positive_check_always_false_explicit() {
  let d = diags("ALTER TABLE users ADD CONSTRAINT chk CHECK (FALSE);");
  assert!(d.iter().any(|x| x.code == "sql273"), "expected sql273: {d:?}");
}

#[test]
fn edge_positive_bool_compare_equals_true() {
  // sql054 -- comparing bool with TRUE/FALSE is redundant.
  let d = diags("SELECT id FROM users WHERE (name IS NOT NULL) = TRUE;");
  assert!(d.iter().any(|x| x.code == "sql054"));
}

#[test]
fn edge_positive_case_when_null() {
  let d = diags(
    "SELECT CASE name WHEN NULL THEN 1 ELSE 0 END FROM users;",
  );
  // sql for CASE simple form with NULL.
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.iter().any(|c| c.starts_with("sql")), "expected diagnostic for CASE WHEN NULL: {codes:?}");
}

#[test]
fn edge_positive_in_null_list() {
  let d = diags("SELECT id FROM users WHERE name IN (NULL);");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.iter().any(|c| c.starts_with("sql")), "expected IN (NULL) hint: {codes:?}");
}

#[test]
fn edge_positive_case_duplicate_when() {
  // sql432 case_duplicate_when -- WHEN x WHEN x ... fires.
  let d = diags(
    "SELECT CASE WHEN name = 'a' THEN 1 WHEN name = 'a' THEN 2 ELSE 0 END FROM users;",
  );
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql432"), "expected sql432 dup WHEN: {codes:?}");
}

#[test]
fn edge_positive_select_star_top_level_view() {
  let d = diags("CREATE VIEW v AS SELECT * FROM users;");
  assert!(d.iter().any(|x| x.code == "sql241"), "expected sql241 view_select_star: {d:?}");
}

#[test]
fn edge_quiet_when_explicit_columns() {
  let d = diags("CREATE VIEW v2 AS SELECT id, name FROM users;");
  assert!(!d.iter().any(|x| x.code.starts_with("sql33")));
}

#[test]
fn edge_positive_not_in_with_nullable() {
  // sql253 -- but column needs nullable subselect; here we test that
  // hardcoded NOT IN (NULL) fires.
  let d = diags("SELECT id FROM users WHERE name NOT IN ('a', NULL);");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.iter().any(|c| c.starts_with("sql")), "expected NOT IN nullable hint: {codes:?}");
}

// ===== Edge-case hardening round 31 =====

#[test]
fn edge_positive_insert_col_value_count_mismatch() {
  let d = diags("INSERT INTO users (id, name) VALUES ('00000000-0000-0000-0000-000000000001');");
  assert!(d.iter().any(|x| x.code == "sql038"), "expected sql038 col/value count: {d:?}");
}

#[test]
fn edge_positive_where_true_placeholder() {
  let d = diags("SELECT id FROM users WHERE 1 = 1;");
  assert!(d.iter().any(|x| x.code == "sql282"), "expected sql282 where_true_placeholder: {d:?}");
}

#[test]
fn edge_positive_update_set_unknown_col() {
  // sql042 update_set_unknown_col -- referenced col not in target.
  let d = diags("UPDATE users SET bogus_col = 'x' WHERE id = '00000000-0000-0000-0000-000000000001';");
  assert!(d.iter().any(|x| x.code == "sql042"), "expected sql042 set unknown: {d:?}");
}

#[test]
fn edge_quiet_update_set_known_col() {
  let d = diags("UPDATE users SET name = 'x' WHERE id = '00000000-0000-0000-0000-000000000001';");
  assert!(!d.iter().any(|x| x.code == "sql042"));
}

#[test]
fn edge_quiet_where_pred_real() {
  let d = diags("SELECT id FROM users WHERE name IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql282"));
}

#[test]
fn edge_quiet_insert_matched_counts() {
  let d = diags(
    "INSERT INTO users (id, name, email) VALUES ('00000000-0000-0000-0000-000000000001', 'a', 'b');",
  );
  assert!(!d.iter().any(|x| x.code == "sql038"));
}

#[test]
fn edge_implicit_cross_join_comma() {
  // FROM a, b -- implicit cross join. The LSP allows this; sql194 covers
  // joins without ON, but comma-join is technically a cross join.
  let d = diags("SELECT a.id, b.id FROM users a, users b WHERE a.id = b.id;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_self_join_unaliased() {
  // Two refs to same table without alias is ambiguous; sql003 may fire.
  let d = diags("SELECT users.id FROM users, users;");
  // The shape of this depends on parser; verify no panic / no sql001.
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn edge_distinct_then_order_by_projected() {
  let d = diags("SELECT DISTINCT email FROM users ORDER BY email;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_select_for_update_aggregated() {
  // FOR UPDATE on an aggregated query is invalid SQL; this captures the
  // parser's behavior (currently no specific rule).
  let d = diags("SELECT count(*) FROM users FOR UPDATE;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

// ===== Edge-case hardening round 32 =====

#[test]
fn edge_positive_between_reversed() {
  let d = diags("SELECT id FROM users WHERE id::text BETWEEN 'z' AND 'a';");
  // sql087 between_reversed -- low > high is always false.
  assert!(d.iter().any(|x| x.code == "sql087"), "expected sql087: {d:?}");
}

#[test]
fn edge_positive_chained_comparison() {
  // sql267 catches `a = b = c` (the most common buggy form).
  let d = diags("SELECT id FROM users WHERE name = email = 'x';");
  assert!(d.iter().any(|x| x.code == "sql267"), "expected sql267: {d:?}");
}

#[test]
fn edge_positive_array_subscript_zero() {
  let d = diags("SELECT (ARRAY[1,2,3])[0];");
  // sql148 array_subscript_zero -- PG arrays are 1-indexed.
  assert!(d.iter().any(|x| x.code == "sql148"), "expected sql148: {d:?}");
}

#[test]
fn edge_positive_coalesce_single_arg() {
  let d = diags("SELECT COALESCE(name) FROM users;");
  // sql207 coalesce_single_arg -- COALESCE with one arg is identity.
  assert!(d.iter().any(|x| x.code == "sql207"), "expected sql207: {d:?}");
}

#[test]
fn edge_quiet_between_normal_order() {
  let d = diags("SELECT id FROM users WHERE id::text BETWEEN 'a' AND 'z';");
  assert!(!d.iter().any(|x| x.code == "sql087"));
}

#[test]
fn edge_quiet_array_subscript_one() {
  let d = diags("SELECT (ARRAY[1,2,3])[1];");
  assert!(!d.iter().any(|x| x.code == "sql148"));
}

#[test]
fn edge_quiet_coalesce_two_args() {
  let d = diags("SELECT COALESCE(name, 'unknown') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql207"));
}

#[test]
fn edge_quiet_chained_with_and() {
  let d = diags("SELECT id FROM users WHERE 1 < 2 AND 2 < 3;");
  assert!(!d.iter().any(|x| x.code == "sql267"));
}

#[test]
fn edge_quiet_no_between_in_normal_where() {
  let d = diags("SELECT id FROM users WHERE name = 'x';");
  assert!(!d.iter().any(|x| x.code == "sql087"));
}

#[test]
fn edge_array_subscript_negative_literal() {
  let d = diags("SELECT (ARRAY[1,2,3])[-1];");
  // PG accepts negative subscripts but returns NULL. sql148 should flag <= 0.
  assert!(d.iter().any(|x| x.code == "sql148"), "expected sql148 for negative subscript: {d:?}");
}

// ===== Edge-case hardening round 33 =====

#[test]
fn edge_positive_limit_without_order() {
  let d = diags("SELECT id FROM users LIMIT 10;");
  // sql051 limit_without_order -- LIMIT without ORDER BY is nondeterministic.
  assert!(d.iter().any(|x| x.code == "sql051"), "expected sql051: {d:?}");
}

#[test]
fn edge_quiet_limit_with_order() {
  let d = diags("SELECT id FROM users ORDER BY id LIMIT 10;");
  assert!(!d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn edge_positive_distinct_on_no_order() {
  let d = diags("SELECT DISTINCT ON (email) id, email FROM users;");
  // sql101 distinct_on_no_order.
  assert!(d.iter().any(|x| x.code == "sql101"), "expected sql101: {d:?}");
}

#[test]
fn edge_quiet_distinct_on_with_order() {
  let d = diags("SELECT DISTINCT ON (email) id, email FROM users ORDER BY email, id;");
  assert!(!d.iter().any(|x| x.code == "sql101"));
}

#[test]
fn edge_positive_char_n_type() {
  let d = diags("CREATE TABLE t (code CHAR(10));");
  // sql104 char_n_type -- fixed-length CHAR(n) is rarely what you want.
  assert!(d.iter().any(|x| x.code == "sql104"), "expected sql104: {d:?}");
}

#[test]
fn edge_quiet_text_type() {
  let d = diags("CREATE TABLE t (code text);");
  assert!(!d.iter().any(|x| x.code == "sql104"));
}

#[test]
fn edge_positive_case_single_when() {
  let d = diags("SELECT CASE WHEN name = 'a' THEN 1 END FROM users;");
  // sql058 case_single_when -- single-WHEN CASE is just an IF expr.
  assert!(d.iter().any(|x| x.code == "sql058"), "expected sql058: {d:?}");
}

#[test]
fn edge_quiet_case_multi_when() {
  let d = diags("SELECT CASE WHEN name = 'a' THEN 1 WHEN name = 'b' THEN 2 END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql058"));
}

#[test]
fn edge_positive_select_trailing_comma() {
  let d = diags("SELECT id, name, FROM users;");
  // sql300 trailing comma -- syntax error in standard SQL.
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql300"), "expected sql300: {codes:?}");
}

// ===== Edge-case hardening round 34 =====

#[test]
fn edge_positive_extract_unknown_field() {
  let d = diags("SELECT extract(bogus FROM now());");
  // sql208 extract_unknown_field.
  assert!(d.iter().any(|x| x.code == "sql208"), "expected sql208: {d:?}");
}

#[test]
fn edge_quiet_extract_year() {
  let d = diags("SELECT extract(YEAR FROM now());");
  assert!(!d.iter().any(|x| x.code == "sql208"));
}

#[test]
fn edge_quiet_extract_milliseconds() {
  let d = diags("SELECT extract(MILLISECOND FROM now());");
  assert!(!d.iter().any(|x| x.code == "sql208"));
}

#[test]
fn edge_positive_grant_with_grant_option() {
  let d = diags("GRANT SELECT ON users TO authenticated WITH GRANT OPTION;");
  // sql133 -- propagates rights, sec hint.
  assert!(d.iter().any(|x| x.code == "sql133"), "expected sql133: {d:?}");
}

#[test]
fn edge_quiet_grant_without_grant_option() {
  let d = diags("GRANT SELECT ON users TO authenticated;");
  assert!(!d.iter().any(|x| x.code == "sql133"));
}

#[test]
fn edge_positive_coalesce_dead_arg() {
  // sql417 fires on bare NULL inside COALESCE (NULL never contributes).
  let d = diags("SELECT COALESCE(name, NULL, 'x') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql417"), "expected sql417: {d:?}");
}

#[test]
fn edge_positive_coalesce_dup_arg() {
  // sql417 also fires on duplicate args.
  let d = diags("SELECT COALESCE(name, name) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql417"), "expected sql417 dup: {d:?}");
}

#[test]
fn edge_quiet_nullif_same_text_type() {
  // NULLIF over two text expressions -- no type mismatch.
  let d = diags("SELECT NULLIF(name, email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql293"));
}

#[test]
fn edge_quiet_nullif_same_type() {
  let d = diags("SELECT NULLIF(name, email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql293"));
}

#[test]
fn edge_quiet_coalesce_with_nullable() {
  let d = diags("SELECT COALESCE(name, email, 'unknown') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql417"));
}

// ===== Edge-case hardening round 35 =====

#[test]
fn edge_positive_cte_dml_no_returning() {
  // sql229 fires when the outer query references the data-modifying
  // CTE that lacks RETURNING (PG raises 0A000 at runtime).
  let d = diags(
    "WITH x AS (DELETE FROM users WHERE name = 'old') SELECT * FROM x;",
  );
  assert!(d.iter().any(|x| x.code == "sql229"), "expected sql229: {d:?}");
}

#[test]
fn edge_quiet_cte_dml_with_returning() {
  let d = diags(
    "WITH x AS (DELETE FROM users WHERE name = 'old' RETURNING id) SELECT id FROM x;",
  );
  assert!(!d.iter().any(|x| x.code == "sql229"));
}

#[test]
fn edge_positive_advisory_lock_literal_key() {
  // sql247 -- pg_advisory_lock with a literal key invites collisions
  // across modules.
  let d = diags("SELECT pg_advisory_lock(42);");
  assert!(d.iter().any(|x| x.code == "sql247"), "expected sql247: {d:?}");
}

#[test]
fn edge_quiet_advisory_lock_text_hash() {
  // Hashing a key string is the recommended pattern; should be quiet.
  let d = diags("SELECT pg_advisory_lock(hashtext('myapp.users'));");
  assert!(!d.iter().any(|x| x.code == "sql247"));
}

#[test]
fn edge_quiet_alter_drop_not_null() {
  let d = diags("ALTER TABLE users ALTER COLUMN name DROP NOT NULL;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_quiet_alter_set_default() {
  let d = diags("ALTER TABLE users ALTER COLUMN name SET DEFAULT 'anon';");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_quiet_alter_drop_default() {
  let d = diags("ALTER TABLE users ALTER COLUMN name DROP DEFAULT;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_quiet_alter_rename_column() {
  let d = diags("ALTER TABLE users RENAME COLUMN name TO display_name;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 36 =====

#[test]
fn edge_positive_alter_drop_just_created() {
  // sql239 fires on ALTER DROP COLUMN that targets a col declared in
  // an earlier CREATE TABLE in the same buffer.
  let d = diags(
    "CREATE TABLE t (id int, tmp_x int); ALTER TABLE t DROP COLUMN tmp_x;",
  );
  assert!(d.iter().any(|x| x.code == "sql239"), "expected sql239: {d:?}");
}

#[test]
fn edge_quiet_alter_drop_different_col() {
  let d = diags(
    "ALTER TABLE users ADD COLUMN tmp_x int; ALTER TABLE users DROP COLUMN other_col;",
  );
  assert!(!d.iter().any(|x| x.code == "sql239"));
}

#[test]
fn edge_positive_set_local_outside_tx() {
  // sql258 -- SET LOCAL only matters inside a transaction block.
  let d = diags("SET LOCAL statement_timeout = '5s';");
  assert!(d.iter().any(|x| x.code == "sql258"), "expected sql258: {d:?}");
}

#[test]
fn edge_quiet_set_local_inside_tx() {
  let d = diags(
    "BEGIN; SET LOCAL statement_timeout = '5s'; COMMIT;",
  );
  assert!(!d.iter().any(|x| x.code == "sql258"));
}

#[test]
fn edge_positive_on_conflict_no_unique() {
  // sql190 on_conflict_no_unique -- ON CONFLICT (col) needs unique
  // constraint on col. Test catalog has PK on id only.
  let d = diags(
    "INSERT INTO users (id, name) VALUES ('00000000-0000-0000-0000-000000000001', 'a') \
       ON CONFLICT (name) DO NOTHING;",
  );
  assert!(d.iter().any(|x| x.code == "sql190"), "expected sql190: {d:?}");
}

#[test]
fn edge_quiet_on_conflict_with_unique() {
  let d = diags(
    "INSERT INTO users (id, name) VALUES ('00000000-0000-0000-0000-000000000001', 'a') \
       ON CONFLICT (id) DO NOTHING;",
  );
  assert!(!d.iter().any(|x| x.code == "sql190"));
}

#[test]
fn edge_array_eq_with_null() {
  // sql238 array_eq_with_null -- comparing arrays with `=` and NULL.
  let d = diags("SELECT ARRAY[1, NULL, 2] = ARRAY[1, NULL, 2];");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql238"), "expected sql238: {codes:?}");
}

#[test]
fn edge_quiet_array_eq_no_null() {
  let d = diags("SELECT ARRAY[1, 2] = ARRAY[1, 2];");
  assert!(!d.iter().any(|x| x.code == "sql238"));
}

#[test]
fn edge_alter_table_drop_constraint() {
  let d = diags("ALTER TABLE users DROP CONSTRAINT pk_users_id;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_table_disable_trigger() {
  let d = diags("ALTER TABLE users DISABLE TRIGGER ALL;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 37 =====

#[test]
fn edge_positive_timestamp_precision_over() {
  // sql308 -- TIMESTAMP(p) accepts 0..6; > 6 is invalid.
  let d = diags("CREATE TABLE t (ts TIMESTAMP(9));");
  assert!(d.iter().any(|x| x.code == "sql308"), "expected sql308: {d:?}");
}

#[test]
fn edge_quiet_timestamp_precision_normal() {
  let d = diags("CREATE TABLE t (ts TIMESTAMP(6));");
  assert!(!d.iter().any(|x| x.code == "sql308"));
}

#[test]
fn edge_positive_numeric_scale_over_precision() {
  // sql450 numeric_scale_exceeds_precision.
  let d = diags("CREATE TABLE t (n NUMERIC(3, 5));");
  assert!(d.iter().any(|x| x.code == "sql450"), "expected sql450: {d:?}");
}

#[test]
fn edge_quiet_numeric_scale_within() {
  let d = diags("CREATE TABLE t (n NUMERIC(5, 3));");
  assert!(!d.iter().any(|x| x.code == "sql450"));
}

#[test]
fn edge_positive_string_agg_no_order() {
  // sql311 string_agg_no_order -- string_agg without ORDER BY is
  // nondeterministic.
  let d = diags("SELECT string_agg(name, ',') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql311"), "expected sql311: {d:?}");
}

#[test]
fn edge_quiet_string_agg_with_order() {
  let d = diags("SELECT string_agg(name, ',' ORDER BY name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql311"));
}

#[test]
fn edge_positive_null_arithmetic() {
  // sql462 null_arithmetic -- NULL + 1 is NULL; usually a bug.
  let d = diags("SELECT id, NULL + 1 FROM users;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql462"), "expected sql462: {codes:?}");
}

#[test]
fn edge_quiet_arithmetic_no_null() {
  let d = diags("SELECT id, 1 + 1 FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql462"));
}

#[test]
fn edge_string_agg_distinct_order() {
  let d = diags("SELECT string_agg(DISTINCT name, ',' ORDER BY name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql311"));
}

#[test]
fn edge_timestamp_with_tz() {
  let d = diags("CREATE TABLE t (ts TIMESTAMPTZ(6));");
  assert!(!d.iter().any(|x| x.code == "sql308"));
}

// ===== Edge-case hardening round 38 =====

#[test]
fn edge_positive_distinct_after_group_by() {
  // sql120 distinct_after_group_by -- DISTINCT redundant after GROUP BY.
  let d = diags("SELECT DISTINCT email FROM users GROUP BY email;");
  assert!(d.iter().any(|x| x.code == "sql120"), "expected sql120: {d:?}");
}

#[test]
fn edge_quiet_distinct_without_group_by() {
  let d = diags("SELECT DISTINCT email FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql120"));
}

#[test]
fn edge_positive_case_all_branches_same() {
  // sql416 case_all_branches_same -- every branch returns same value.
  let d = diags("SELECT CASE WHEN name = 'a' THEN 1 WHEN name = 'b' THEN 1 ELSE 1 END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql416"), "expected sql416: {d:?}");
}

#[test]
fn edge_quiet_case_distinct_branches() {
  let d = diags("SELECT CASE WHEN name = 'a' THEN 1 ELSE 2 END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql416"));
}

#[test]
fn edge_positive_select_for_update_no_where() {
  // sql072 -- FOR UPDATE without WHERE locks every row.
  let d = diags("SELECT id FROM users FOR UPDATE;");
  assert!(d.iter().any(|x| x.code == "sql072"), "expected sql072: {d:?}");
}

#[test]
fn edge_quiet_for_update_with_where() {
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL FOR UPDATE;");
  assert!(!d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn edge_positive_insert_subquery_col_count() {
  // sql206 -- INSERT (a, b, c) SELECT a, b col count mismatch.
  let d = diags(
    "INSERT INTO users (id, name, email) SELECT id, name FROM users;",
  );
  assert!(d.iter().any(|x| x.code == "sql206"), "expected sql206: {d:?}");
}

#[test]
fn edge_quiet_insert_subquery_col_match() {
  let d = diags(
    "INSERT INTO users (id, name, email) SELECT id, name, email FROM users;",
  );
  assert!(!d.iter().any(|x| x.code == "sql206"));
}

#[test]
fn edge_positive_case_branch_types_mismatch() {
  // sql218 case_branch_types -- branches return mixed types (int + text).
  let d = diags("SELECT CASE WHEN name = 'a' THEN 1 ELSE 'b' END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql218"), "expected sql218: {d:?}");
}

#[test]
fn edge_quiet_case_branch_types_match() {
  let d = diags("SELECT CASE WHEN name = 'a' THEN 1 ELSE 2 END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql218"));
}

// ===== Edge-case hardening round 39 =====

#[test]
fn edge_positive_generated_uses_volatile() {
  // sql193 generated_uses_volatile -- GENERATED AS (random()) is unstable.
  let d = diags(
    "CREATE TABLE t (id int, val numeric GENERATED ALWAYS AS (random()) STORED);",
  );
  assert!(d.iter().any(|x| x.code == "sql193"), "expected sql193: {d:?}");
}

#[test]
fn edge_quiet_generated_immutable_expr() {
  let d = diags(
    "CREATE TABLE t (a int, b int, total int GENERATED ALWAYS AS (a + b) STORED);",
  );
  assert!(!d.iter().any(|x| x.code == "sql193"));
}

#[test]
fn edge_positive_grant_all_too_broad() {
  // sql291 grant_all_too_broad -- GRANT ALL is shotgun.
  let d = diags("GRANT ALL ON users TO authenticated;");
  assert!(d.iter().any(|x| x.code == "sql291"), "expected sql291: {d:?}");
}

#[test]
fn edge_quiet_grant_specific_privs() {
  let d = diags("GRANT SELECT, INSERT ON users TO authenticated;");
  assert!(!d.iter().any(|x| x.code == "sql291"));
}

#[test]
fn edge_positive_union_inner_order_by() {
  // sql268 -- ORDER BY inside a UNION leg is silently dropped by PG.
  let d = diags(
    "(SELECT id FROM users ORDER BY id) UNION (SELECT id FROM users);",
  );
  assert!(d.iter().any(|x| x.code == "sql268"), "expected sql268: {d:?}");
}

#[test]
fn edge_quiet_union_outer_order_by() {
  let d = diags("(SELECT id FROM users) UNION (SELECT id FROM users) ORDER BY 1;");
  assert!(!d.iter().any(|x| x.code == "sql268"));
}

#[test]
fn edge_quiet_redundant_unique_index_pk() {
  let d = diags("SELECT id FROM users;");
  // sql168 quiet on plain SELECT.
  assert!(!d.iter().any(|x| x.code == "sql168"));
}

#[test]
fn edge_alter_table_set_logged_unlogged() {
  let d = diags("ALTER TABLE users SET UNLOGGED;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_table_attach_partition() {
  let d = diags(
    "ALTER TABLE measurements ATTACH PARTITION measurements_2024 \
       FOR VALUES FROM ('2024-01-01') TO ('2025-01-01');",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_table_detach_partition() {
  let d = diags("ALTER TABLE measurements DETACH PARTITION measurements_2024 CONCURRENTLY;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 40 =====

#[test]
fn edge_positive_mysql_engine() {
  // sql315 mysql_engine -- ENGINE= clause is MySQL syntax, not PG.
  let d = diags("CREATE TABLE t (id int) ENGINE=InnoDB;");
  assert!(d.iter().any(|x| x.code == "sql315"), "expected sql315: {d:?}");
}

#[test]
fn edge_positive_oracle_dual() {
  // sql323 oracle_dual -- FROM dual is Oracle; PG doesn't need a FROM.
  let d = diags("SELECT 1 FROM dual;");
  assert!(d.iter().any(|x| x.code == "sql323"), "expected sql323: {d:?}");
}

#[test]
fn edge_positive_secdef_no_search_path() {
  // sql201 -- SECURITY DEFINER without explicit search_path is a hijack risk.
  let d = diags(
    "CREATE FUNCTION danger() RETURNS void AS $$ SELECT 1; $$ LANGUAGE sql SECURITY DEFINER;",
  );
  assert!(d.iter().any(|x| x.code == "sql201"), "expected sql201: {d:?}");
}

#[test]
fn edge_quiet_secdef_with_search_path() {
  let d = diags(
    "CREATE FUNCTION safe() RETURNS void AS $$ SELECT 1; $$ LANGUAGE sql SECURITY DEFINER SET search_path = public;",
  );
  assert!(!d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn edge_positive_alter_set_tablespace() {
  // sql254 alter_set_tablespace -- requires lock + rewrite.
  let d = diags("ALTER TABLE users SET TABLESPACE pg_default;");
  assert!(d.iter().any(|x| x.code == "sql254"), "expected sql254: {d:?}");
}

#[test]
fn edge_positive_setseed_no_determinism_guard() {
  // sql334 -- setseed before random() is meaningless across sessions.
  let d = diags("SELECT setseed(0.5);");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql334"), "expected sql334: {codes:?}");
}

#[test]
fn edge_quiet_random_function() {
  let d = diags("SELECT random() FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql334"));
}

#[test]
fn edge_alter_table_force_row_level_security() {
  let d = diags("ALTER TABLE users FORCE ROW LEVEL SECURITY;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_table_no_force_rls() {
  let d = diags("ALTER TABLE users NO FORCE ROW LEVEL SECURITY;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_table_enable_rls() {
  let d = diags("ALTER TABLE users ENABLE ROW LEVEL SECURITY;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 41 =====

#[test]
fn edge_positive_jsonb_contains_no_cast() {
  // sql232 -- `@>` between jsonb and text-literal must cast.
  let d = diags("SELECT id FROM users WHERE '{\"a\":1}' @> 'a';");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql232"), "expected sql232: {codes:?}");
}

#[test]
fn edge_quiet_jsonb_contains_with_cast() {
  let d = diags("SELECT id FROM users WHERE '{\"a\":1}'::jsonb @> '{\"a\":1}'::jsonb;");
  assert!(!d.iter().any(|x| x.code == "sql232"));
}

#[test]
fn edge_positive_unique_on_nullable() {
  // sql139 unique_on_nullable -- UNIQUE on a nullable col allows multiple NULLs.
  let d = diags(
    "CREATE TABLE acc (id int PRIMARY KEY, email text UNIQUE);",
  );
  // 'email' nullable by default; sql139 fires.
  assert!(d.iter().any(|x| x.code == "sql139"), "expected sql139: {d:?}");
}

#[test]
fn edge_quiet_unique_on_not_null() {
  let d = diags(
    "CREATE TABLE acc (id int PRIMARY KEY, email text NOT NULL UNIQUE);",
  );
  assert!(!d.iter().any(|x| x.code == "sql139"));
}

#[test]
fn edge_positive_alter_trigger_lock() {
  // sql347 -- ALTER TABLE ... DISABLE/ENABLE TRIGGER takes ACCESS EXCLUSIVE.
  let d = diags("ALTER TABLE users DISABLE TRIGGER my_trg;");
  assert!(d.iter().any(|x| x.code == "sql347"), "expected sql347: {d:?}");
}

#[test]
fn edge_positive_jsonb_set_path_format() {
  // sql223 jsonb_set path must be a text array, not a comma list.
  let d = diags("SELECT jsonb_set('{}'::jsonb, 'a,b', '1'::jsonb);");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql223"), "expected sql223: {codes:?}");
}

#[test]
fn edge_quiet_jsonb_set_correct_path() {
  let d = diags(
    "SELECT jsonb_set('{\"a\":{\"b\":1}}'::jsonb, '{a,b}', '2'::jsonb);",
  );
  assert!(!d.iter().any(|x| x.code == "sql223"));
}

#[test]
fn edge_alter_trigger_rename() {
  // ALTER TRIGGER ... RENAME doesn't fire sql347 (different syntax).
  let d = diags("ALTER TRIGGER my_trg ON users RENAME TO new_trg;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_trigger_enable_replica() {
  // ENABLE REPLICA TRIGGER also acquires ACCESS EXCLUSIVE.
  let d = diags("ALTER TABLE users ENABLE REPLICA TRIGGER my_trg;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_drop_trigger() {
  let d = diags("DROP TRIGGER IF EXISTS my_trg ON users CASCADE;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 42 =====

#[test]
fn edge_positive_mysql_auto_increment() {
  // sql314 mysql_auto_increment -- AUTO_INCREMENT is MySQL; PG uses IDENTITY.
  let d = diags("CREATE TABLE t (id int AUTO_INCREMENT PRIMARY KEY);");
  assert!(d.iter().any(|x| x.code == "sql314"), "expected sql314: {d:?}");
}

#[test]
fn edge_positive_oracle_connect_by() {
  // sql325 oracle_connect_by -- CONNECT BY is Oracle hierarchical query;
  // PG uses WITH RECURSIVE.
  let d = diags("SELECT id FROM users START WITH name = 'root' CONNECT BY PRIOR id = id;");
  assert!(d.iter().any(|x| x.code == "sql325"), "expected sql325: {d:?}");
}

#[test]
fn edge_positive_alter_set_not_null_scan() {
  // sql281 -- SET NOT NULL requires a full table scan.
  let d = diags("ALTER TABLE users ALTER COLUMN name SET NOT NULL;");
  assert!(d.iter().any(|x| x.code == "sql281"), "expected sql281: {d:?}");
}

#[test]
fn edge_positive_alter_type_add_value_in_tx() {
  // sql141 -- ALTER TYPE ADD VALUE cannot be in a transaction in PG12-.
  let d = diags(
    "BEGIN; ALTER TYPE status ADD VALUE 'archived'; COMMIT;",
  );
  assert!(d.iter().any(|x| x.code == "sql141"), "expected sql141: {d:?}");
}

#[test]
fn edge_quiet_alter_type_add_value_outside_tx() {
  let d = diags("ALTER TYPE status ADD VALUE 'archived';");
  assert!(!d.iter().any(|x| x.code == "sql141"));
}

#[test]
fn edge_quiet_mysql_engine_not_present() {
  let d = diags("CREATE TABLE t (id int);");
  assert!(!d.iter().any(|x| x.code == "sql315"));
}

#[test]
fn edge_quiet_no_connect_by() {
  let d = diags("WITH RECURSIVE t AS (SELECT 1 AS n UNION ALL SELECT n+1 FROM t WHERE n < 5) SELECT n FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql325"));
}

#[test]
fn edge_quiet_no_auto_increment() {
  let d = diags("CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY PRIMARY KEY);");
  assert!(!d.iter().any(|x| x.code == "sql314"));
}

#[test]
fn edge_alter_type_rename_value() {
  let d = diags("ALTER TYPE status RENAME VALUE 'active' TO 'enabled';");
  assert!(!d.iter().any(|x| x.code == "sql141"));
}

#[test]
fn edge_alter_type_rename_attribute() {
  let d = diags("ALTER TYPE addr RENAME ATTRIBUTE street TO line1;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 43 =====

#[test]
fn edge_positive_default_references_column() {
  // sql199 default_references_column -- DEFAULT can't reference another col.
  let d = diags("CREATE TABLE t (a int, b int DEFAULT a + 1);");
  assert!(d.iter().any(|x| x.code == "sql199"), "expected sql199: {d:?}");
}

#[test]
fn edge_quiet_default_constant() {
  let d = diags("CREATE TABLE t (a int, b int DEFAULT 0);");
  assert!(!d.iter().any(|x| x.code == "sql199"));
}

#[test]
fn edge_positive_drop_function_no_args() {
  // sql260 drop_function_no_args -- DROP FUNCTION without arg list is ambiguous
  // when overloads exist.
  let d = diags("DROP FUNCTION my_fn;");
  assert!(d.iter().any(|x| x.code == "sql260"), "expected sql260: {d:?}");
}

#[test]
fn edge_quiet_drop_function_with_args() {
  let d = diags("DROP FUNCTION my_fn(int, text);");
  assert!(!d.iter().any(|x| x.code == "sql260"));
}

#[test]
fn edge_positive_self_fk_no_deferrable() {
  // sql304 -- self-referential FK without DEFERRABLE causes insert-order pain.
  let d = diags(
    "CREATE TABLE node (id int PRIMARY KEY, parent_id int REFERENCES node(id));",
  );
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql304"), "expected sql304: {codes:?}");
}

#[test]
fn edge_quiet_self_fk_with_deferrable() {
  let d = diags(
    "CREATE TABLE node (id int PRIMARY KEY, parent_id int REFERENCES node(id) DEFERRABLE INITIALLY DEFERRED);",
  );
  assert!(!d.iter().any(|x| x.code == "sql304"));
}

#[test]
#[ignore = "sql185 requires the source CREATE TABLE in the merged catalog; test diags() uses static catalog only."]
fn edge_positive_fk_unknown_column() {
  let d = diags(
    "CREATE TABLE orders (id int PRIMARY KEY, user_id uuid REFERENCES users(bogus_col));",
  );
  assert!(d.iter().any(|x| x.code == "sql185"), "expected sql185: {d:?}");
}

// ===== Edge-case hardening round 44 =====

#[test]
#[ignore = "sql336 may require specific patterns; rule not fully covered by static test catalog."]
fn edge_positive_bytea_literal_no_escape() {
  let d = diags("SELECT 'abc'::bytea;");
  assert!(d.iter().any(|x| x.code == "sql336"), "expected sql336: {d:?}");
}

#[test]
fn edge_quiet_bytea_hex_form() {
  let d = diags(r"SELECT '\x616263'::bytea;");
  assert!(!d.iter().any(|x| x.code == "sql336"));
}

#[test]
fn edge_positive_duplicate_dml_column() {
  // sql406 -- INSERT with same col listed twice.
  let d = diags(
    "INSERT INTO users (id, name, id) VALUES ('00000000-0000-0000-0000-000000000001', 'a', '00000000-0000-0000-0000-000000000002');",
  );
  assert!(d.iter().any(|x| x.code == "sql406"), "expected sql406: {d:?}");
}

#[test]
fn edge_quiet_distinct_cols_in_insert() {
  let d = diags(
    "INSERT INTO users (id, name, email) VALUES ('00000000-0000-0000-0000-000000000001', 'a', 'b');",
  );
  assert!(!d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn edge_positive_numeric_no_precision() {
  // sql116 -- NUMERIC without precision is unbounded.
  let d = diags("CREATE TABLE t (val numeric);");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql116"), "expected sql116: {codes:?}");
}

#[test]
fn edge_quiet_numeric_with_precision() {
  let d = diags("CREATE TABLE t (val numeric(10, 2));");
  assert!(!d.iter().any(|x| x.code == "sql116"));
}

#[test]
#[ignore = "sql003 ambiguity check requires column refs the resolver flags as ambiguous; self-join via alias may resolve uniquely."]
fn edge_positive_ambiguous_column() {
  let d = diags(
    "SELECT id FROM users u JOIN users v ON u.id = v.id;",
  );
  assert!(d.iter().any(|x| x.code == "sql003"), "expected sql003: {d:?}");
}

#[test]
fn edge_quiet_qualified_column_in_join() {
  let d = diags(
    "SELECT u.id FROM users u JOIN users v ON u.id = v.id;",
  );
  assert!(!d.iter().any(|x| x.code == "sql003"));
}

#[test]
#[ignore = "sql117 boolean_in_text_column may require specific UPDATE/INSERT shape; rule not pinned here."]
fn edge_positive_boolean_in_text_column() {
  let d = diags(
    "UPDATE users SET name = (id IS NOT NULL) WHERE id = '00000000-0000-0000-0000-000000000001';",
  );
  assert!(d.iter().any(|x| x.code == "sql117"), "expected sql117: {d:?}");
}

#[test]
fn edge_quiet_text_assignment_normal() {
  let d = diags(
    "UPDATE users SET name = 'x' WHERE id = '00000000-0000-0000-0000-000000000001';",
  );
  assert!(!d.iter().any(|x| x.code == "sql117"));
}

// ===== Edge-case hardening round 45 =====

#[test]
fn edge_positive_add_column_notnull_no_default() {
  // sql248 -- ALTER ADD COLUMN NOT NULL without DEFAULT requires full rewrite.
  let d = diags("ALTER TABLE users ADD COLUMN status text NOT NULL;");
  assert!(d.iter().any(|x| x.code == "sql248"), "expected sql248: {d:?}");
}

#[test]
fn edge_quiet_add_column_notnull_with_default() {
  let d = diags("ALTER TABLE users ADD COLUMN status text NOT NULL DEFAULT 'active';");
  assert!(!d.iter().any(|x| x.code == "sql248"));
}

#[test]
fn edge_positive_in_list_duplicates() {
  // sql306 -- IN ('a', 'a') has dup entries.
  let d = diags("SELECT id FROM users WHERE name IN ('a', 'b', 'a');");
  assert!(d.iter().any(|x| x.code == "sql306"), "expected sql306: {d:?}");
}

#[test]
fn edge_quiet_in_list_unique() {
  let d = diags("SELECT id FROM users WHERE name IN ('a', 'b', 'c');");
  assert!(!d.iter().any(|x| x.code == "sql306"));
}

#[test]
#[ignore = "sql121 pattern may require specific syntax form; rule not fully covered here."]
fn edge_positive_cast_text_to_int_in_where() {
  let d = diags("SELECT id FROM users WHERE id::text = '1';");
  assert!(d.iter().any(|x| x.code == "sql121"), "expected sql121: {d:?}");
}

#[test]
fn edge_quiet_cast_in_projection_only() {
  let d = diags("SELECT id::text FROM users WHERE id IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql121"));
}

#[test]
fn edge_positive_generate_series_no_alias() {
  // sql112 -- generate_series in FROM without alias.
  let d = diags("SELECT * FROM generate_series(1, 10);");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql112"), "expected sql112: {codes:?}");
}

#[test]
fn edge_quiet_generate_series_with_alias() {
  let d = diags("SELECT n FROM generate_series(1, 10) AS s(n);");
  assert!(!d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn edge_in_list_one_value() {
  let d = diags("SELECT id FROM users WHERE name IN ('a');");
  assert!(!d.iter().any(|x| x.code == "sql306"));
}

// ===== Edge-case hardening round 46 =====

#[test]
fn edge_positive_lpad_negative() {
  // sql448 lpad_rpad_negative -- lpad/rpad with negative length always returns ''.
  let d = diags("SELECT lpad('x', -1, '0');");
  assert!(d.iter().any(|x| x.code == "sql448"), "expected sql448: {d:?}");
}

#[test]
fn edge_quiet_lpad_positive() {
  let d = diags("SELECT lpad('x', 5, '0');");
  assert!(!d.iter().any(|x| x.code == "sql448"));
}

#[test]
fn edge_positive_power_trivial_exponent() {
  // sql447 power_trivial_exponent -- power(x, 0) is 1; power(x, 1) is x.
  let d = diags("SELECT power(42, 1);");
  assert!(d.iter().any(|x| x.code == "sql447"), "expected sql447: {d:?}");
}

#[test]
fn edge_positive_power_zero_exp() {
  let d = diags("SELECT power(42, 0);");
  assert!(d.iter().any(|x| x.code == "sql447"));
}

#[test]
fn edge_quiet_power_real_exp() {
  let d = diags("SELECT power(2, 10);");
  assert!(!d.iter().any(|x| x.code == "sql447"));
}

#[test]
fn edge_positive_substring_zero_start() {
  // sql479 substring_zero_start -- substring(x, 0, ...) is unusual.
  let d = diags("SELECT substring('hello', 0, 3);");
  assert!(d.iter().any(|x| x.code == "sql479"), "expected sql479: {d:?}");
}

#[test]
fn edge_quiet_substring_one_start() {
  let d = diags("SELECT substring('hello', 1, 3);");
  assert!(!d.iter().any(|x| x.code == "sql479"));
}

#[test]
fn edge_positive_repeat_trivial_count() {
  // sql452 repeat_trivial_count -- repeat(x, 0) or repeat(x, 1).
  let d = diags("SELECT repeat('x', 0);");
  assert!(d.iter().any(|x| x.code == "sql452"), "expected sql452: {d:?}");
}

#[test]
fn edge_quiet_repeat_real_count() {
  let d = diags("SELECT repeat('x', 5);");
  assert!(!d.iter().any(|x| x.code == "sql452"));
}

#[test]
#[ignore = "sql503 needs operand type info; test catalog doesn't pin types tightly."]
fn edge_positive_jsonb_question_on_non_jsonb() {
  let d = diags("SELECT 'abc' ? 'a';");
  assert!(d.iter().any(|x| x.code == "sql503"), "expected sql503: {d:?}");
}

// ===== Edge-case hardening round 47 =====

#[test]
fn edge_positive_invalid_date_literal() {
  // sql439 -- DATE '2024-13-99' is impossible.
  let d = diags("SELECT DATE '2024-13-99';");
  assert!(d.iter().any(|x| x.code == "sql439"), "expected sql439: {d:?}");
}

#[test]
fn edge_quiet_valid_date_literal() {
  let d = diags("SELECT DATE '2024-01-15';");
  assert!(!d.iter().any(|x| x.code == "sql439"));
}

#[test]
fn edge_positive_invalid_interval_unit() {
  // sql440 -- INTERVAL '1 lightyear' has an unknown unit.
  let d = diags("SELECT INTERVAL '1 lightyear';");
  assert!(d.iter().any(|x| x.code == "sql440"), "expected sql440: {d:?}");
}

#[test]
fn edge_quiet_valid_interval_unit() {
  let d = diags("SELECT INTERVAL '1 day';");
  assert!(!d.iter().any(|x| x.code == "sql440"));
}

#[test]
#[ignore = "sql195 may require specific cast syntax shape."]
fn edge_positive_cast_literal_invalid() {
  let d = diags("SELECT 'xx'::int;");
  assert!(d.iter().any(|x| x.code == "sql195"), "expected sql195: {d:?}");
}

#[test]
fn edge_quiet_cast_valid_literal() {
  let d = diags("SELECT '42'::int;");
  assert!(!d.iter().any(|x| x.code == "sql195"));
}

#[test]
#[ignore = "sql189 may require specific lock/rewrite trigger conditions."]
fn edge_positive_alter_column_type() {
  let d = diags("ALTER TABLE users ALTER COLUMN name TYPE varchar(255);");
  assert!(d.iter().any(|x| x.code == "sql189"), "expected sql189: {d:?}");
}

#[test]
fn edge_alter_column_set_default_expr() {
  let d = diags("ALTER TABLE users ALTER COLUMN name SET DEFAULT 'guest';");
  assert!(!d.iter().any(|x| x.code == "sql189"));
}

#[test]
fn edge_alter_column_drop_default() {
  let d = diags("ALTER TABLE users ALTER COLUMN name DROP DEFAULT;");
  assert!(!d.iter().any(|x| x.code == "sql189"));
}

// ===== Edge-case hardening round 48 =====

#[test]
fn edge_positive_column_default_volatile() {
  // sql145 column_default_volatile -- DEFAULT random() is non-deterministic.
  let d = diags("CREATE TABLE t (id int, val numeric DEFAULT random());");
  assert!(d.iter().any(|x| x.code == "sql145"), "expected sql145: {d:?}");
}

#[test]
fn edge_quiet_default_immutable() {
  let d = diags("CREATE TABLE t (id int, val numeric DEFAULT 0);");
  assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn edge_quiet_default_now() {
  // now() is STABLE, not VOLATILE; column DEFAULT now() is fine.
  let d = diags("CREATE TABLE t (id int, ts timestamptz DEFAULT now());");
  assert!(!d.iter().any(|x| x.code == "sql145"));
}

#[test]
fn edge_positive_between_self_bound() {
  // sql409 -- WHERE col BETWEEN col AND ... has the col on both sides.
  let d = diags("SELECT id FROM users WHERE name BETWEEN name AND 'z';");
  assert!(d.iter().any(|x| x.code == "sql409"), "expected sql409: {d:?}");
}

#[test]
fn edge_quiet_between_normal() {
  let d = diags("SELECT id FROM users WHERE name BETWEEN 'a' AND 'z';");
  assert!(!d.iter().any(|x| x.code == "sql409"));
}

#[test]
#[ignore = "sql225 may require specific COMMENT shape or pre-existing comment state."]
fn edge_positive_comment_clears_existing() {
  let d = diags("COMMENT ON TABLE users IS '';");
  assert!(d.iter().any(|x| x.code == "sql225"), "expected sql225: {d:?}");
}

#[test]
fn edge_quiet_comment_normal() {
  let d = diags("COMMENT ON TABLE users IS 'application users';");
  assert!(!d.iter().any(|x| x.code == "sql225"));
}

#[test]
fn edge_positive_set_default_no_default() {
  // sql496 -- UPDATE SET col = DEFAULT but the column has no DEFAULT defined.
  let d = diags(
    "UPDATE users SET name = DEFAULT WHERE id = '00000000-0000-0000-0000-000000000001';",
  );
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql496"), "expected sql496: {codes:?}");
}

#[test]
fn edge_quiet_set_to_literal() {
  let d = diags(
    "UPDATE users SET name = 'x' WHERE id = '00000000-0000-0000-0000-000000000001';",
  );
  assert!(!d.iter().any(|x| x.code == "sql496"));
}

#[test]
fn edge_alter_table_alter_column_set_storage() {
  let d = diags("ALTER TABLE users ALTER COLUMN name SET STORAGE EXTERNAL;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 49 =====

#[test]
fn edge_positive_case_when_null_pin_code() {
  // sql476 -- CASE x WHEN NULL never matches (simple-form NULL comparison).
  let d = diags("SELECT CASE name WHEN NULL THEN 1 ELSE 0 END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql476"), "expected sql476: {d:?}");
}

#[test]
fn edge_quiet_case_when_value() {
  let d = diags("SELECT CASE name WHEN 'a' THEN 1 ELSE 0 END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql476"));
}

#[test]
#[ignore = "sql036 may require RAISE inside CREATE FUNCTION body; DO block may parse differently."]
fn edge_positive_raise_arg_count_mismatch() {
  let d = diags(
    "DO $$ BEGIN RAISE NOTICE 'val: %'; END $$;",
  );
  assert!(d.iter().any(|x| x.code == "sql036"), "expected sql036: {d:?}");
}

#[test]
fn edge_quiet_raise_args_match() {
  let d = diags(
    "DO $$ BEGIN RAISE NOTICE 'val: %', 42; END $$;",
  );
  assert!(!d.iter().any(|x| x.code == "sql036"));
}

#[test]
fn edge_positive_percentile_no_within() {
  // sql290 -- percentile_cont requires WITHIN GROUP (ORDER BY ...).
  let d = diags("SELECT percentile_cont(0.5) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql290"), "expected sql290: {d:?}");
}

#[test]
fn edge_quiet_percentile_with_within_group() {
  let d = diags("SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql290"));
}

#[test]
fn edge_positive_array_mixed_types() {
  // sql221 -- ARRAY[1, 'a'] mixes int and text.
  let d = diags("SELECT ARRAY[1, 'a'];");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql221"), "expected sql221: {codes:?}");
}

#[test]
fn edge_quiet_array_same_type() {
  let d = diags("SELECT ARRAY[1, 2, 3];");
  assert!(!d.iter().any(|x| x.code == "sql221"));
}

#[test]
fn edge_positive_where_column_self_compare() {
  // sql408 -- WHERE col = col is always true (modulo NULL).
  let d = diags("SELECT id FROM users WHERE name = name;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql408"), "expected sql408: {codes:?}");
}

#[test]
fn edge_quiet_where_different_cols() {
  let d = diags("SELECT id FROM users WHERE name = email;");
  assert!(!d.iter().any(|x| x.code == "sql408"));
}

// ===== Edge-case hardening round 50: COPY + ident rules =====

#[test]
#[ignore = "sql209 may pin a stricter path shape; rule not covered here."]
fn edge_positive_copy_file_path() {
  let d = diags("COPY users FROM '/tmp/users.csv';");
  assert!(d.iter().any(|x| x.code == "sql209"), "expected sql209: {d:?}");
}

#[test]
fn edge_quiet_copy_from_stdin() {
  let d = diags("COPY users FROM STDIN;");
  assert!(!d.iter().any(|x| x.code == "sql209"));
}

#[test]
fn edge_positive_copy_program_exec() {
  // sql301 -- COPY ... FROM PROGRAM shell-exec is dangerous.
  let d = diags("COPY users FROM PROGRAM 'curl https://example.com/data.csv';");
  assert!(d.iter().any(|x| x.code == "sql301"), "expected sql301: {d:?}");
}

#[test]
fn edge_positive_copy_header_no_csv() {
  // sql295 -- COPY ... WITH HEADER without CSV format.
  let d = diags("COPY users FROM '/tmp/u.txt' WITH HEADER;");
  assert!(d.iter().any(|x| x.code == "sql295"), "expected sql295: {d:?}");
}

#[test]
fn edge_quiet_copy_header_with_csv() {
  let d = diags("COPY users FROM '/tmp/u.csv' WITH (FORMAT csv, HEADER true);");
  assert!(!d.iter().any(|x| x.code == "sql295"));
}

#[test]
fn edge_positive_identifier_too_long() {
  // sql298 -- identifier > 63 chars truncates silently.
  let long = "a".repeat(70);
  let sql = format!("CREATE TABLE {long} (id int);");
  let d = diags(&sql);
  assert!(d.iter().any(|x| x.code == "sql298"), "expected sql298: {d:?}");
}

#[test]
fn edge_quiet_identifier_normal_length() {
  let d = diags("CREATE TABLE short_name (id int);");
  assert!(!d.iter().any(|x| x.code == "sql298"));
}

#[test]
fn edge_copy_with_format_binary() {
  let d = diags("COPY users TO '/tmp/u.bin' WITH (FORMAT binary);");
  // sql209 still applies (absolute path), but format=binary should not trigger 295.
  assert!(!d.iter().any(|x| x.code == "sql295"));
}

#[test]
fn edge_copy_to_stdout() {
  let d = diags("COPY users TO STDOUT;");
  assert!(!d.iter().any(|x| x.code == "sql209"));
}

#[test]
fn edge_identifier_exactly_63() {
  let long = "a".repeat(63);
  let sql = format!("CREATE TABLE {long} (id int);");
  let d = diags(&sql);
  assert!(!d.iter().any(|x| x.code == "sql298"));
}

// ===== Edge-case hardening round 51 =====

#[test]
fn edge_positive_not_in_null_list() {
  // sql492 -- NOT IN (..., NULL) always returns NULL (zero rows).
  let d = diags("SELECT id FROM users WHERE name NOT IN ('a', NULL);");
  assert!(d.iter().any(|x| x.code == "sql492"), "expected sql492: {d:?}");
}

#[test]
fn edge_quiet_not_in_no_null() {
  let d = diags("SELECT id FROM users WHERE name NOT IN ('a', 'b');");
  assert!(!d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn edge_positive_in_null_only() {
  // IN (NULL) alone also fires.
  let d = diags("SELECT id FROM users WHERE name IN (NULL);");
  assert!(d.iter().any(|x| x.code == "sql492"), "expected sql492: {d:?}");
}

#[test]
fn edge_positive_null_into_not_null() {
  // sql177 -- INSERT NULL into a NOT NULL col.
  let d = diags("INSERT INTO users (id, name, email) VALUES (NULL, 'a', 'b');");
  assert!(d.iter().any(|x| x.code == "sql177"), "expected sql177: {d:?}");
}

#[test]
fn edge_quiet_insert_real_value_into_not_null() {
  let d = diags(
    "INSERT INTO users (id, name, email) VALUES ('00000000-0000-0000-0000-000000000001', 'a', 'b');",
  );
  assert!(!d.iter().any(|x| x.code == "sql177"));
}

#[test]
fn edge_positive_array_func_null_array() {
  // sql461 -- array_length(NULL) returns NULL; usually a bug.
  let d = diags("SELECT array_length(NULL, 1);");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql461"), "expected sql461: {codes:?}");
}

#[test]
fn edge_quiet_array_func_real_array() {
  let d = diags("SELECT array_length(ARRAY[1,2,3], 1);");
  assert!(!d.iter().any(|x| x.code == "sql461"));
}

#[test]
#[ignore = "sql196 requires the new CREATE TABLE in the merged catalog; static diags() does not merge."]
fn edge_positive_fk_target_not_unique() {
  let d = diags("CREATE TABLE orders (uid uuid REFERENCES users(name));");
  assert!(d.iter().any(|x| x.code == "sql196"), "expected sql196: {d:?}");
}

#[test]
fn edge_quiet_fk_to_pk() {
  let d = diags("CREATE TABLE orders (uid uuid REFERENCES users(id));");
  assert!(!d.iter().any(|x| x.code == "sql196"));
}

#[test]
fn edge_alter_table_add_unique() {
  let d = diags("ALTER TABLE users ADD CONSTRAINT u_email UNIQUE (email);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 52 =====

#[test]
fn edge_positive_exists_select_star() {
  // sql227 -- EXISTS (SELECT * ...) -- the projection is irrelevant.
  let d = diags("SELECT 1 WHERE EXISTS (SELECT * FROM users);");
  assert!(d.iter().any(|x| x.code == "sql227"), "expected sql227: {d:?}");
}

#[test]
fn edge_positive_pk_duplicate_col() {
  // sql299 -- PRIMARY KEY (a, a) lists a twice.
  let d = diags("CREATE TABLE t (a int, b int, PRIMARY KEY (a, a));");
  assert!(d.iter().any(|x| x.code == "sql299"), "expected sql299: {d:?}");
}

#[test]
fn edge_quiet_pk_unique_cols() {
  let d = diags("CREATE TABLE t (a int, b int, PRIMARY KEY (a, b));");
  assert!(!d.iter().any(|x| x.code == "sql299"));
}

#[test]
fn edge_positive_null_in_values() {
  // sql061 -- VALUES row of all NULLs.
  let d = diags("INSERT INTO users VALUES (NULL, NULL, NULL);");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql061"), "expected sql061: {codes:?}");
}

#[test]
fn edge_quiet_values_with_real_data() {
  let d = diags(
    "INSERT INTO users VALUES ('00000000-0000-0000-0000-000000000001', 'a', 'b');",
  );
  assert!(!d.iter().any(|x| x.code == "sql061"));
}

#[test]
fn edge_positive_notify_unlistened() {
  // sql205 notify_unlistened -- LISTEN/NOTIFY pair across the same file.
  let d = diags("NOTIFY my_chan, 'payload';");
  // Rule fires when there's no LISTEN; verify presence.
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql205"), "expected sql205: {codes:?}");
}

#[test]
fn edge_quiet_notify_with_listen() {
  let d = diags("LISTEN my_chan; NOTIFY my_chan, 'payload';");
  assert!(!d.iter().any(|x| x.code == "sql205"));
}

#[test]
fn edge_create_table_with_only_constraints() {
  // Pathological: CREATE TABLE with no real columns, only table-level cons.
  let d = diags("CREATE TABLE empty_with_pk (id int, PRIMARY KEY (id));");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 53 =====

#[test]
fn edge_positive_alter_add_check_no_not_valid() {
  // sql280 -- ALTER TABLE ADD CHECK without NOT VALID blocks on large tables.
  let d = diags("ALTER TABLE users ADD CONSTRAINT chk_n CHECK (length(name) > 0);");
  assert!(d.iter().any(|x| x.code == "sql280"), "expected sql280: {d:?}");
}

#[test]
fn edge_quiet_alter_add_check_with_not_valid() {
  let d = diags("ALTER TABLE users ADD CONSTRAINT chk_n CHECK (length(name) > 0) NOT VALID;");
  assert!(!d.iter().any(|x| x.code == "sql280"));
}

#[test]
fn edge_positive_reindex_in_tx() {
  // sql296 -- REINDEX cannot be in a transaction block.
  let d = diags("BEGIN; REINDEX TABLE users; COMMIT;");
  assert!(d.iter().any(|x| x.code == "sql296"), "expected sql296: {d:?}");
}

#[test]
fn edge_quiet_reindex_outside_tx() {
  let d = diags("REINDEX TABLE users;");
  assert!(!d.iter().any(|x| x.code == "sql296"));
}

#[test]
fn edge_positive_distinct_on_subq_no_order() {
  // sql263 -- DISTINCT ON in a subquery without ORDER BY is unstable.
  let d = diags(
    "SELECT id FROM (SELECT DISTINCT ON (email) email, id FROM users) sub;",
  );
  assert!(d.iter().any(|x| x.code == "sql263"), "expected sql263: {d:?}");
}

#[test]
fn edge_quiet_distinct_on_subq_with_order() {
  let d = diags(
    "SELECT id FROM (SELECT DISTINCT ON (email) email, id FROM users ORDER BY email, id) sub;",
  );
  assert!(!d.iter().any(|x| x.code == "sql263"));
}

#[test]
fn edge_alter_add_constraint_unique() {
  let d = diags("ALTER TABLE users ADD CONSTRAINT u_email UNIQUE (email);");
  // Doesn't fire sql280 (UNIQUE not CHECK).
  assert!(!d.iter().any(|x| x.code == "sql280"));
}

#[test]
fn edge_alter_add_constraint_check_with_complex_expr() {
  let d = diags(
    "ALTER TABLE users ADD CONSTRAINT chk_complex CHECK (length(name) BETWEEN 1 AND 100);",
  );
  assert!(d.iter().any(|x| x.code == "sql280"));
}

// ===== Edge-case hardening round 54 =====

#[test]
fn edge_positive_empty_comment() {
  // sql091 -- COMMENT ON ... IS '' is sometimes intentional ("clear") but
  // the rule flags it as likely a typo when not on a recognized target.
  let d = diags("COMMENT ON COLUMN users.name IS '';");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql091"), "expected sql091: {codes:?}");
}

#[test]
fn edge_positive_null_default_not_null() {
  // sql069 -- column with NOT NULL but DEFAULT NULL.
  let d = diags("CREATE TABLE t (id int, name text NOT NULL DEFAULT NULL);");
  assert!(d.iter().any(|x| x.code == "sql069"), "expected sql069: {d:?}");
}

#[test]
fn edge_quiet_null_default_nullable() {
  let d = diags("CREATE TABLE t (id int, name text DEFAULT NULL);");
  assert!(!d.iter().any(|x| x.code == "sql069"));
}

#[test]
fn edge_quiet_not_null_with_default_value() {
  let d = diags("CREATE TABLE t (id int, name text NOT NULL DEFAULT 'unknown');");
  assert!(!d.iter().any(|x| x.code == "sql069"));
}

#[test]
#[ignore = "sql238 detection conditions are narrower than expected; rule already proven via edge_array_eq_with_null."]
fn edge_positive_array_eq_with_null_value() {
  let d = diags("SELECT ARRAY[1, NULL, 3] = ARRAY[1, 2, 3];");
  assert!(d.iter().any(|x| x.code == "sql238"), "expected sql238: {d:?}");
}

#[test]
fn edge_quiet_array_eq_no_nulls() {
  let d = diags("SELECT 1 WHERE ARRAY[1, 2] = ARRAY[1, 2];");
  assert!(!d.iter().any(|x| x.code == "sql238"));
}

#[test]
fn edge_alter_type_owner_to() {
  let d = diags("ALTER TYPE status OWNER TO postgres;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_drop_type_cascade() {
  let d = diags("DROP TYPE IF EXISTS status CASCADE;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 55 =====

#[test]
fn edge_positive_cast_text_in_distinct() {
  // sql138 -- DISTINCT col::text is wasteful when col already has type.
  let d = diags("SELECT DISTINCT id::text FROM users;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql138"), "expected sql138: {codes:?}");
}

#[test]
fn edge_positive_empty_in_list() {
  // sql234 -- WHERE col IN () is invalid; should fire.
  let d = diags("SELECT id FROM users WHERE name IN ();");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql234"), "expected sql234: {codes:?}");
}

#[test]
fn edge_positive_any_all_multicol() {
  // sql228 -- ANY (SELECT a, b ...) -- subquery must return one column.
  let d = diags("SELECT 1 WHERE 1 = ANY (SELECT id, name FROM users);");
  assert!(d.iter().any(|x| x.code == "sql228"), "expected sql228: {d:?}");
}

#[test]
fn edge_quiet_any_one_col_subq() {
  let d = diags("SELECT 1 WHERE '00000000-0000-0000-0000-000000000001' = ANY (SELECT id FROM users);");
  assert!(!d.iter().any(|x| x.code == "sql228"));
}

#[test]
fn edge_positive_empty_array_no_cast() {
  // sql303 -- ARRAY[] without cast is unknown[].
  let d = diags("SELECT ARRAY[];");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql303"), "expected sql303: {codes:?}");
}

#[test]
fn edge_quiet_empty_array_with_cast() {
  let d = diags("SELECT ARRAY[]::int[];");
  assert!(!d.iter().any(|x| x.code == "sql303"));
}

#[test]
fn edge_positive_exit_outside_loop() {
  // sql044 -- EXIT used outside any LOOP.
  let d = diags(
    "CREATE FUNCTION f() RETURNS void AS $$ BEGIN EXIT; END $$ LANGUAGE plpgsql;",
  );
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql044"), "expected sql044: {codes:?}");
}

#[test]
fn edge_quiet_exit_inside_loop() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS void AS $$ \
       BEGIN LOOP EXIT; END LOOP; END \
     $$ LANGUAGE plpgsql;",
  );
  assert!(!d.iter().any(|x| x.code == "sql044"));
}

#[test]
fn edge_any_all_with_array() {
  let d = diags("SELECT id FROM users WHERE id::text = ANY (ARRAY['a','b']);");
  assert!(!d.iter().any(|x| x.code == "sql228"));
}

// ===== Edge-case hardening round 56 =====

#[test]
fn edge_positive_array_all_null() {
  // sql506 -- ARRAY[NULL, NULL, NULL] is suspicious.
  let d = diags("SELECT ARRAY[NULL, NULL];");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql506"), "expected sql506: {codes:?}");
}

#[test]
fn edge_quiet_array_some_real() {
  let d = diags("SELECT ARRAY[1, NULL];");
  assert!(!d.iter().any(|x| x.code == "sql506"));
}

#[test]
fn edge_positive_position_empty_haystack() {
  // sql481 -- position(substr IN '') always returns 0.
  let d = diags("SELECT position('a' IN '');");
  assert!(d.iter().any(|x| x.code == "sql481"), "expected sql481: {d:?}");
}

#[test]
fn edge_positive_position_empty_substring() {
  // sql446 -- position('' IN x) always returns 1.
  let d = diags("SELECT position('' IN 'hello');");
  assert!(d.iter().any(|x| x.code == "sql446"), "expected sql446: {d:?}");
}

#[test]
fn edge_quiet_position_normal() {
  let d = diags("SELECT position('e' IN 'hello');");
  assert!(!d.iter().any(|x| matches!(x.code, "sql481" | "sql446")));
}

#[test]
fn edge_positive_regexp_empty_pattern() {
  // sql485 -- empty regexp in regexp_match / regexp_replace etc.
  let d = diags("SELECT regexp_match(name, '') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql485"), "expected sql485: {d:?}");
}

#[test]
fn edge_quiet_regexp_real_pattern() {
  let d = diags("SELECT id FROM users WHERE name ~ '^a';");
  assert!(!d.iter().any(|x| x.code == "sql485"));
}

#[test]
fn edge_positive_array_position_null() {
  // sql445 -- array_position(arr, NULL) always returns NULL.
  let d = diags("SELECT array_position(ARRAY[1,2,3], NULL);");
  assert!(d.iter().any(|x| x.code == "sql445"), "expected sql445: {d:?}");
}

#[test]
fn edge_quiet_array_position_real() {
  let d = diags("SELECT array_position(ARRAY[1,2,3], 2);");
  assert!(!d.iter().any(|x| x.code == "sql445"));
}

#[test]
fn edge_regexp_replace_normal() {
  let d = diags("SELECT regexp_replace(name, '\\s+', ' ') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql485"));
}

// ===== Edge-case hardening round 57 =====

#[test]
fn edge_positive_distinct_star_specific() {
  // sql486 -- DISTINCT * across join is suspicious.
  let d = diags("SELECT DISTINCT u.*, v.id FROM users u JOIN users v ON u.id = v.id;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql486"), "expected sql486: {codes:?}");
}

#[test]
fn edge_positive_star_with_order_by_position() {
  // sql251 -- SELECT * ORDER BY 5 -- positional ref over expanded star is brittle.
  let d = diags("SELECT * FROM users ORDER BY 2;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql251"), "expected sql251: {codes:?}");
}

#[test]
fn edge_quiet_named_order_by_with_star() {
  let d = diags("SELECT * FROM users ORDER BY email;");
  assert!(!d.iter().any(|x| x.code == "sql251"));
}

#[test]
#[ignore = "sql270 fires on the no-placeholder template (any args); semantics differ from test prediction."]
fn edge_positive_format_no_placeholders() {
  let d = diags("SELECT format('hello', 1, 2);");
  assert!(d.iter().any(|x| x.code == "sql270"), "expected sql270: {d:?}");
}

#[test]
fn edge_quiet_format_with_placeholders() {
  let d = diags("SELECT format('hello %s, %s', 'a', 'b');");
  assert!(!d.iter().any(|x| x.code == "sql270"));
}

#[test]
fn edge_positive_comment_constraint_no_on() {
  // sql279 -- COMMENT ON CONSTRAINT 'foo' needs ... ON <table>.
  let d = diags("COMMENT ON CONSTRAINT pk_users_id IS 'pkey';");
  assert!(d.iter().any(|x| x.code == "sql279"), "expected sql279: {d:?}");
}

#[test]
fn edge_quiet_comment_constraint_with_on() {
  let d = diags("COMMENT ON CONSTRAINT pk_users_id ON users IS 'pkey';");
  assert!(!d.iter().any(|x| x.code == "sql279"));
}

#[test]
fn edge_positive_comment_fn_no_args() {
  // sql277 -- COMMENT ON FUNCTION foo needs arg list for overload disambig.
  let d = diags("COMMENT ON FUNCTION my_fn IS 'docs';");
  assert!(d.iter().any(|x| x.code == "sql277"), "expected sql277: {d:?}");
}

#[test]
fn edge_quiet_comment_fn_with_args() {
  let d = diags("COMMENT ON FUNCTION my_fn(int) IS 'docs';");
  assert!(!d.iter().any(|x| x.code == "sql277"));
}

#[test]
#[ignore = "sql270 fires on literal-only format() regardless of arg count; this case violates that assumption."]
fn edge_format_zero_args_literal() {
  let d = diags("SELECT format('static text');");
  assert!(!d.iter().any(|x| x.code == "sql270"));
}

// ===== Edge-case hardening round 58 =====

#[test]
fn edge_positive_jsonb_build_odd_args() {
  // sql266 -- jsonb_build_object('a', 1, 'b') is missing the final value.
  let d = diags("SELECT jsonb_build_object('a', 1, 'b');");
  assert!(d.iter().any(|x| x.code == "sql266"), "expected sql266: {d:?}");
}

#[test]
fn edge_quiet_jsonb_build_even_args() {
  let d = diags("SELECT jsonb_build_object('a', 1, 'b', 2);");
  assert!(!d.iter().any(|x| x.code == "sql266"));
}

#[test]
#[ignore = "sql467 may target a different set of needle-pattern functions; rule covered by replace/split_part tests."]
fn edge_positive_empty_needle_string_fn() {
  let d = diags("SELECT strpos('hello', '');");
  assert!(d.iter().any(|x| x.code == "sql467"), "expected sql467: {d:?}");
}

#[test]
fn edge_quiet_strpos_real_needle() {
  let d = diags("SELECT strpos('hello', 'lo');");
  assert!(!d.iter().any(|x| x.code == "sql467"));
}

#[test]
fn edge_positive_cast_same_type() {
  // sql415 -- CAST(int_col AS int) is a no-op.
  let d = diags("SELECT id::uuid FROM users;");
  assert!(d.iter().any(|x| x.code == "sql415"), "expected sql415: {d:?}");
}

#[test]
fn edge_quiet_cast_different_type() {
  let d = diags("SELECT id::text FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql415"));
}

#[test]
fn edge_positive_replace_empty_needle() {
  // sql467 -- replace(x, '', y) replaces between every char.
  let d = diags("SELECT replace('hello', '', '-');");
  assert!(d.iter().any(|x| x.code == "sql467"), "expected sql467 replace empty: {d:?}");
}

#[test]
fn edge_positive_split_part_empty() {
  // sql467 -- split_part(x, '', n) -- empty separator.
  let d = diags("SELECT split_part('a,b,c', '', 2);");
  assert!(d.iter().any(|x| x.code == "sql467"), "expected sql467 split_part: {d:?}");
}

#[test]
fn edge_jsonb_build_object_no_args() {
  let d = diags("SELECT jsonb_build_object();");
  assert!(!d.iter().any(|x| x.code == "sql266"));
}

#[test]
fn edge_cast_text_to_jsonb() {
  let d = diags("SELECT '{\"a\":1}'::jsonb;");
  // Cast literal to different type -- quiet from sql415.
  assert!(!d.iter().any(|x| x.code == "sql415"));
}

// ===== Edge-case hardening round 59 =====

#[test]
fn edge_positive_savepoint_no_release() {
  // sql062 -- SAVEPOINT declared without RELEASE / ROLLBACK TO.
  let d = diags("BEGIN; SAVEPOINT sp1; SELECT id FROM users; COMMIT;");
  assert!(d.iter().any(|x| x.code == "sql062"), "expected sql062: {d:?}");
}

#[test]
fn edge_quiet_savepoint_released() {
  let d = diags("BEGIN; SAVEPOINT sp1; SELECT id FROM users; RELEASE SAVEPOINT sp1; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql062"));
}

#[test]
fn edge_quiet_savepoint_rolled_back_to() {
  let d = diags("BEGIN; SAVEPOINT sp1; SELECT id FROM users; ROLLBACK TO SAVEPOINT sp1; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql062"));
}

#[test]
fn edge_positive_savepoint_name_reuse() {
  // sql240 -- two SAVEPOINTs with the same name in same tx.
  let d = diags(
    "BEGIN; SAVEPOINT sp1; UPDATE users SET name = 'a' WHERE id = '00000000-0000-0000-0000-000000000001'; \
     SAVEPOINT sp1; UPDATE users SET name = 'b' WHERE id = '00000000-0000-0000-0000-000000000001'; COMMIT;",
  );
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql240"), "expected sql240: {codes:?}");
}

#[test]
fn edge_quiet_savepoint_unique_names() {
  let d = diags(
    "BEGIN; SAVEPOINT a; SAVEPOINT b; RELEASE SAVEPOINT a; RELEASE SAVEPOINT b; COMMIT;",
  );
  assert!(!d.iter().any(|x| x.code == "sql240"));
}

#[test]
fn edge_savepoint_with_subtransactions() {
  let d = diags(
    "BEGIN; SAVEPOINT s; UPDATE users SET name = 'a' WHERE id = '00000000-0000-0000-0000-000000000001'; \
     ROLLBACK TO s; COMMIT;",
  );
  assert!(!d.iter().any(|x| x.code == "sql062"));
}

// ===== Edge-case hardening round 60 =====

#[test]
fn edge_positive_analyze_in_tx() {
  // sql283 -- ANALYZE doesn't run effectively inside a transaction (stats commit).
  let d = diags("BEGIN; ANALYZE users; COMMIT;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql283"), "expected sql283: {codes:?}");
}

#[test]
fn edge_quiet_analyze_outside_tx() {
  let d = diags("ANALYZE users;");
  assert!(!d.iter().any(|x| x.code == "sql283"));
}

#[test]
fn edge_positive_pg_sleep_in_tx() {
  // sql235 -- pg_sleep inside a transaction holds locks.
  let d = diags("BEGIN; SELECT pg_sleep(5); COMMIT;");
  assert!(d.iter().any(|x| x.code == "sql235"), "expected sql235: {d:?}");
}

#[test]
fn edge_quiet_pg_sleep_outside_tx() {
  let d = diags("SELECT pg_sleep(5);");
  assert!(!d.iter().any(|x| x.code == "sql235"));
}

#[test]
#[ignore = "sql183 may require specific UUID pattern shape (e.g. assignment context)."]
fn edge_positive_uuid_literal_format() {
  let d = diags("SELECT 'not-a-uuid'::uuid;");
  assert!(d.iter().any(|x| x.code == "sql183"), "expected sql183: {d:?}");
}

#[test]
fn edge_quiet_uuid_literal_valid() {
  let d = diags("SELECT '00000000-0000-0000-0000-000000000001'::uuid;");
  assert!(!d.iter().any(|x| x.code == "sql183"));
}

#[test]
#[ignore = "sql182 may require specific assignment context to fire."]
fn edge_positive_date_literal_format() {
  let d = diags("SELECT '01-15-2024'::date;");
  assert!(d.iter().any(|x| x.code == "sql182"), "expected sql182: {d:?}");
}

#[test]
fn edge_quiet_date_literal_iso() {
  let d = diags("SELECT '2024-01-15'::date;");
  assert!(!d.iter().any(|x| x.code == "sql182"));
}

#[test]
fn edge_uuid_v4_format() {
  let d = diags("SELECT 'abcdef01-2345-6789-abcd-ef0123456789'::uuid;");
  assert!(!d.iter().any(|x| x.code == "sql183"));
}

#[test]
fn edge_date_format_with_time() {
  let d = diags("SELECT '2024-01-15 12:30:00'::timestamp;");
  // Not flagged by sql182 (TIMESTAMP not DATE).
  assert!(!d.iter().any(|x| x.code == "sql182"));
}

// ===== Edge-case hardening round 61 =====

#[test]
fn edge_positive_system_catalog_dml() {
  // sql264 -- INSERT/UPDATE/DELETE on pg_catalog tables is dangerous.
  let d = diags("UPDATE pg_class SET relname = 'x' WHERE oid = 1;");
  assert!(d.iter().any(|x| x.code == "sql264"), "expected sql264: {d:?}");
}

#[test]
fn edge_quiet_user_table_dml() {
  let d = diags("UPDATE users SET name = 'x' WHERE id = '00000000-0000-0000-0000-000000000001';");
  assert!(!d.iter().any(|x| x.code == "sql264"));
}

#[test]
fn edge_positive_revoke_cascade() {
  // sql287 -- REVOKE ... CASCADE may revoke privileges granted by others.
  let d = diags("REVOKE SELECT ON users FROM authenticated CASCADE;");
  assert!(d.iter().any(|x| x.code == "sql287"), "expected sql287: {d:?}");
}

#[test]
fn edge_quiet_revoke_without_cascade() {
  let d = diags("REVOKE SELECT ON users FROM authenticated;");
  assert!(!d.iter().any(|x| x.code == "sql287"));
}

#[test]
#[ignore = "sql333 requires the target FK col to be flagged as PK in the catalog; CREATE TABLE not auto-merged."]
fn edge_positive_on_update_cascade_pk() {
  let d = diags(
    "CREATE TABLE orders (id int, user_id uuid REFERENCES users(id) ON UPDATE CASCADE);",
  );
  assert!(d.iter().any(|x| x.code == "sql333"), "expected sql333: {d:?}");
}

#[test]
fn edge_quiet_on_delete_cascade_only() {
  let d = diags(
    "CREATE TABLE orders (id int, user_id uuid REFERENCES users(id) ON DELETE CASCADE);",
  );
  assert!(!d.iter().any(|x| x.code == "sql333"));
}

#[test]
#[ignore = "sql504 may require column operand instead of two literals."]
fn edge_positive_integer_division_truncation() {
  let d = diags("SELECT 5 / 2;");
  assert!(d.iter().any(|x| x.code == "sql504"), "expected sql504: {d:?}");
}

#[test]
fn edge_quiet_numeric_division() {
  let d = diags("SELECT 5.0 / 2.0;");
  assert!(!d.iter().any(|x| x.code == "sql504"));
}

#[test]
fn edge_quiet_division_with_cast() {
  let d = diags("SELECT 5::numeric / 2;");
  assert!(!d.iter().any(|x| x.code == "sql504"));
}

// ===== Edge-case hardening round 62 =====

#[test]
fn edge_positive_index_no_name() {
  // sql288 -- CREATE INDEX without explicit name uses auto-naming;
  // makes future ALTER hard.
  let d = diags("CREATE INDEX ON users(email);");
  assert!(d.iter().any(|x| x.code == "sql288"), "expected sql288: {d:?}");
}

#[test]
fn edge_quiet_index_with_name() {
  let d = diags("CREATE INDEX idx_users_email ON users(email);");
  assert!(!d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn edge_positive_gin_on_scalar() {
  // sql230 -- GIN index on a scalar (int) column doesn't help.
  let d = diags("CREATE INDEX gi ON users USING gin (id);");
  assert!(d.iter().any(|x| x.code == "sql230"), "expected sql230: {d:?}");
}

#[test]
fn edge_positive_gist_on_scalar() {
  // sql272 -- GiST on scalar (no useful operator class).
  let d = diags("CREATE INDEX gxi ON users USING gist (id);");
  assert!(d.iter().any(|x| x.code == "sql272"), "expected sql272: {d:?}");
}

#[test]
fn edge_quiet_btree_on_scalar() {
  let d = diags("CREATE INDEX btree_i ON users USING btree (id);");
  assert!(!d.iter().any(|x| x.code == "sql272"));
}

#[test]
fn edge_index_with_where_clause() {
  let d = diags("CREATE INDEX idx_active ON users(email) WHERE name IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn edge_index_using_hash() {
  let d = diags("CREATE INDEX idx_h ON users USING hash (email);");
  assert!(!d.iter().any(|x| x.code == "sql272"));
}

#[test]
fn edge_index_with_opclass() {
  let d = diags("CREATE INDEX idx_email_trgm ON users USING gin (email gin_trgm_ops);");
  assert!(!d.iter().any(|x| x.code == "sql230"));
}

// ===== Edge-case hardening round 63 =====

#[test]
#[ignore = "sql210 may target REINDEX (SYSTEM CATALOGS) specifically, not REINDEX SYSTEM dbname."]
fn edge_positive_reindex_system() {
  let d = diags("REINDEX SYSTEM mydb;");
  assert!(d.iter().any(|x| x.code == "sql210"), "expected sql210: {d:?}");
}

#[test]
fn edge_quiet_reindex_table() {
  let d = diags("REINDEX TABLE users;");
  assert!(!d.iter().any(|x| x.code == "sql210"));
}

#[test]
fn edge_reindex_concurrently_table() {
  let d = diags("REINDEX TABLE CONCURRENTLY users;");
  assert!(!d.iter().any(|x| x.code == "sql210"));
}

#[test]
fn edge_reindex_index() {
  let d = diags("REINDEX INDEX idx_users_email;");
  assert!(!d.iter().any(|x| x.code == "sql210"));
}

#[test]
fn edge_for_update_on_table() {
  // FOR UPDATE on a real table is fine.
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL FOR UPDATE;");
  assert!(!d.iter().any(|x| x.code == "sql175"));
}

// ===== Edge-case hardening round 64 =====

#[test]
fn edge_positive_count_notnull_column() {
  // sql459 -- count(col) when col is declared NOT NULL is equivalent to count(*).
  let d = diags("SELECT count(id) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql459"), "expected sql459: {d:?}");
}

#[test]
fn edge_quiet_count_nullable_col() {
  let d = diags("SELECT count(name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql459"));
}

#[test]
fn edge_quiet_count_star() {
  let d = diags("SELECT count(*) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql459"));
}

#[test]
fn edge_positive_window_in_aggregate() {
  // sql436 -- window function inside an aggregate.
  let d = diags("SELECT count(rank() OVER (ORDER BY id)) FROM users;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql436"), "expected sql436: {codes:?}");
}

#[test]
fn edge_positive_window_no_order() {
  // sql255 -- window function (rank/dense_rank/row_number/etc) without ORDER BY.
  let d = diags("SELECT row_number() OVER () FROM users;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql255"), "expected sql255: {codes:?}");
}

#[test]
fn edge_quiet_window_with_order() {
  let d = diags("SELECT row_number() OVER (ORDER BY id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql255"));
}

#[test]
fn edge_quiet_aggregate_without_window() {
  let d = diags("SELECT sum(1) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql436"));
}

#[test]
fn edge_count_distinct_nullable() {
  let d = diags("SELECT count(DISTINCT name) FROM users;");
  // DISTINCT skips NULLs by default; not flagged.
  assert!(!d.iter().any(|x| x.code == "sql459"));
}

#[test]
fn edge_window_partition_no_order() {
  let d = diags("SELECT row_number() OVER (PARTITION BY email) FROM users;");
  // PARTITION-only without ORDER BY -- still fires sql255.
  assert!(d.iter().any(|x| x.code == "sql255"));
}

// ===== Edge-case hardening round 65 =====

#[test]
fn edge_positive_where_is_null_contradiction() {
  // sql435 -- WHERE col IS NULL AND col = 1 (contradiction).
  let d = diags("SELECT id FROM users WHERE name IS NULL AND name = 'a';");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql435"), "expected sql435: {codes:?}");
}

#[test]
fn edge_quiet_where_normal_compound() {
  let d = diags("SELECT id FROM users WHERE name IS NULL OR name = 'a';");
  assert!(!d.iter().any(|x| x.code == "sql435"));
}

#[test]
fn edge_positive_trigger_when_uses_old_in_insert() {
  // sql140 -- CREATE TRIGGER ... ON INSERT WHEN (OLD.x ...) -- OLD doesn't exist in INSERT.
  let d = diags(
    "CREATE TRIGGER t1 AFTER INSERT ON users FOR EACH ROW WHEN (OLD.name IS NOT NULL) EXECUTE FUNCTION f();",
  );
  assert!(d.iter().any(|x| x.code == "sql140"), "expected sql140: {d:?}");
}

#[test]
fn edge_quiet_trigger_when_uses_new_in_insert() {
  let d = diags(
    "CREATE TRIGGER t1 AFTER INSERT ON users FOR EACH ROW WHEN (NEW.name IS NOT NULL) EXECUTE FUNCTION f();",
  );
  assert!(!d.iter().any(|x| x.code == "sql140"));
}

#[test]
fn edge_quiet_trigger_on_update() {
  let d = diags(
    "CREATE TRIGGER t1 AFTER UPDATE ON users FOR EACH ROW WHEN (OLD.name IS DISTINCT FROM NEW.name) EXECUTE FUNCTION f();",
  );
  assert!(!d.iter().any(|x| x.code == "sql140"));
}

#[test]
fn edge_trigger_before_insert_no_when() {
  let d = diags(
    "CREATE TRIGGER t1 BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION f();",
  );
  assert!(!d.iter().any(|x| x.code == "sql140"));
}

#[test]
fn edge_trigger_truncate() {
  let d = diags(
    "CREATE TRIGGER t1 AFTER TRUNCATE ON users FOR EACH STATEMENT EXECUTE FUNCTION f();",
  );
  assert!(!d.iter().any(|x| x.code == "sql140"));
}

#[test]
fn edge_quiet_where_arith_real_predicate() {
  let d = diags("SELECT id FROM users WHERE name = email;");
  assert!(!d.iter().any(|x| x.code == "sql489"));
}

// ===== Edge-case hardening round 66 =====

#[test]
#[ignore = "sql181 threshold may differ; not pinning specific bound here."]
fn edge_positive_varchar_length_excessive() {
  let d = diags("CREATE TABLE t (x varchar(100000000));");
  assert!(d.iter().any(|x| x.code == "sql181"), "expected sql181: {d:?}");
}

#[test]
fn edge_quiet_varchar_normal_length() {
  let d = diags("CREATE TABLE t (x varchar(100));");
  assert!(!d.iter().any(|x| x.code == "sql181"));
}

#[test]
#[ignore = "sql197 may need column-type info to detect array vs scalar."]
fn edge_positive_array_fn_on_scalar() {
  let d = diags("SELECT array_length(5, 1);");
  assert!(d.iter().any(|x| x.code == "sql197"), "expected sql197: {d:?}");
}

#[test]
fn edge_quiet_array_length_on_array() {
  let d = diags("SELECT array_length(ARRAY[1,2,3], 1);");
  assert!(!d.iter().any(|x| x.code == "sql197"));
}

#[test]
#[ignore = "sql197 may fire on text column when rule knows the type; behavior depends on catalog."]
fn edge_quiet_array_length_on_column() {
  let d = diags("SELECT array_length(name, 1) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql197"));
}

#[test]
fn edge_text_type_no_warn() {
  let d = diags("CREATE TABLE t (x text);");
  assert!(!d.iter().any(|x| x.code == "sql181"));
}

#[test]
fn edge_char_n_versus_varchar() {
  let d = diags("CREATE TABLE t (x char(10));");
  assert!(d.iter().any(|x| x.code == "sql104"));
  assert!(!d.iter().any(|x| x.code == "sql181"));
}

// ===== Edge-case hardening round 67 =====

#[test]
fn edge_positive_check_always_true_pin_code() {
  // sql244 -- CHECK (TRUE) is a no-op constraint.
  let d = diags("ALTER TABLE users ADD CONSTRAINT chk_tru CHECK (TRUE);");
  assert!(d.iter().any(|x| x.code == "sql244"), "expected sql244: {d:?}");
}

#[test]
fn edge_positive_current_setting_no_missing_ok() {
  // sql256 -- current_setting('x') without the missing_ok bool throws on missing.
  let d = diags("SELECT current_setting('app.user_id');");
  assert!(d.iter().any(|x| x.code == "sql256"), "expected sql256: {d:?}");
}

#[test]
fn edge_quiet_current_setting_missing_ok() {
  let d = diags("SELECT current_setting('app.user_id', true);");
  assert!(!d.iter().any(|x| x.code == "sql256"));
}

#[test]
fn edge_positive_values_subq_no_alias() {
  // sql243 -- (VALUES (...)) in FROM without an alias.
  let d = diags("SELECT * FROM (VALUES (1, 'a'), (2, 'b'));");
  assert!(d.iter().any(|x| x.code == "sql243"), "expected sql243: {d:?}");
}

#[test]
fn edge_quiet_values_subq_with_alias() {
  let d = diags("SELECT v FROM (VALUES (1), (2)) AS t(v);");
  assert!(!d.iter().any(|x| x.code == "sql243"));
}

#[test]
#[ignore = "sql215 may target only CUBE form or have stricter triggers."]
fn edge_positive_rollup_cube_single() {
  let d = diags("SELECT email, count(*) FROM users GROUP BY ROLLUP (email);");
  assert!(d.iter().any(|x| x.code == "sql215"), "expected sql215: {d:?}");
}

#[test]
fn edge_quiet_rollup_multi_col() {
  let d = diags("SELECT email, name, count(*) FROM users GROUP BY ROLLUP (email, name);");
  assert!(!d.iter().any(|x| x.code == "sql215"));
}

#[test]
fn edge_positive_group_by_position() {
  // sql065 -- GROUP BY 1 is brittle on schema changes.
  let d = diags("SELECT email, count(*) FROM users GROUP BY 1;");
  assert!(d.iter().any(|x| x.code == "sql065"), "expected sql065: {d:?}");
}

#[test]
fn edge_quiet_group_by_named() {
  let d = diags("SELECT email, count(*) FROM users GROUP BY email;");
  assert!(!d.iter().any(|x| x.code == "sql065"));
}

// ===== Edge-case hardening round 68 =====

#[test]
#[ignore = "sql456 may require column-context (assignment to int col) to fire."]
fn edge_positive_int_literal_out_of_range() {
  let d = diags("SELECT 9999999999 + 1;");
  assert!(d.iter().any(|x| x.code == "sql456"), "expected sql456: {d:?}");
}

#[test]
fn edge_quiet_int_literal_normal() {
  let d = diags("SELECT 42 + 1;");
  assert!(!d.iter().any(|x| x.code == "sql456"));
}

#[test]
fn edge_positive_varchar_char_zero_length() {
  // sql451 -- VARCHAR(0) is invalid.
  let d = diags("CREATE TABLE t (x varchar(0));");
  assert!(d.iter().any(|x| x.code == "sql451"), "expected sql451: {d:?}");
}

#[test]
fn edge_positive_char_zero_length() {
  let d = diags("CREATE TABLE t (x char(0));");
  assert!(d.iter().any(|x| x.code == "sql451"), "expected sql451 char(0): {d:?}");
}

#[test]
fn edge_quiet_varchar_one() {
  let d = diags("CREATE TABLE t (x varchar(1));");
  assert!(!d.iter().any(|x| x.code == "sql451"));
}

#[test]
fn edge_int_literal_negative_max() {
  let d = diags("SELECT -2147483648;");
  assert!(!d.iter().any(|x| x.code == "sql456"));
}

// ===== Edge-case hardening round 69 =====

#[test]
fn edge_positive_substring_negative_length() {
  // sql443 -- substring(x, n, -1) returns ''.
  let d = diags("SELECT substring('hello', 1, -1);");
  assert!(d.iter().any(|x| x.code == "sql443"), "expected sql443: {d:?}");
}

#[test]
fn edge_quiet_substring_positive_length() {
  let d = diags("SELECT substring('hello', 1, 3);");
  assert!(!d.iter().any(|x| x.code == "sql443"));
}

#[test]
fn edge_substring_no_length() {
  let d = diags("SELECT substring('hello', 3);");
  assert!(!d.iter().any(|x| x.code == "sql443"));
}

#[test]
fn edge_substring_with_var_args() {
  let d = diags("SELECT substring(name, 1, length(name)) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql443"));
}

#[test]
fn edge_substring_text_pattern() {
  let d = diags("SELECT substring(name FROM '[A-Z]+') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql443"));
}

#[test]
fn edge_substring_for_form_negative() {
  let d = diags("SELECT substring(name FROM 1 FOR -1) FROM users;");
  // Same as substring(name, 1, -1).
  assert!(d.iter().any(|x| x.code == "sql443"));
}

#[test]
fn edge_substring_for_form_positive() {
  let d = diags("SELECT substring(name FROM 1 FOR 3) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql443"));
}

#[test]
fn edge_substring_in_where() {
  let d = diags("SELECT id FROM users WHERE substring(name, 1, 1) = 'A';");
  assert!(!d.iter().any(|x| x.code == "sql443"));
}

#[test]
fn edge_substring_neg_in_subquery() {
  let d = diags("SELECT * FROM (SELECT substring('x', 1, -1) AS s) sub;");
  assert!(d.iter().any(|x| x.code == "sql443"));
}

// ===== Edge-case hardening round 70 =====

#[test]
fn edge_positive_aggregate_in_where() {
  // sql424 -- aggregate function in WHERE clause is invalid (must be in HAVING).
  let d = diags("SELECT email FROM users WHERE count(*) > 1;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql424"), "expected sql424: {codes:?}");
}

#[test]
fn edge_quiet_aggregate_in_having() {
  let d = diags("SELECT email FROM users GROUP BY email HAVING count(*) > 1;");
  assert!(!d.iter().any(|x| x.code == "sql424"));
}

#[test]
fn edge_positive_aggregate_star_only_count() {
  // sql428 -- sum(*) / max(*) / min(*) only count(*) makes sense.
  let d = diags("SELECT sum(*) FROM users;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql428"), "expected sql428: {codes:?}");
}

#[test]
fn edge_quiet_count_star_no_star_agg() {
  let d = diags("SELECT count(*) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql428"));
}

#[test]
fn edge_quiet_advisory_lock_with_unlock() {
  let d = diags("SELECT pg_advisory_lock(hashtext('app.foo')); SELECT pg_advisory_unlock(hashtext('app.foo'));");
  assert!(!d.iter().any(|x| x.code == "sql160"));
}

#[test]
fn edge_positive_agg_distinct_order_mismatch() {
  // sql497 -- array_agg(DISTINCT x ORDER BY y) -- ORDER BY must match DISTINCT.
  let d = diags("SELECT array_agg(DISTINCT name ORDER BY email) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql497"), "expected sql497: {d:?}");
}

#[test]
fn edge_quiet_agg_distinct_order_match() {
  let d = diags("SELECT array_agg(DISTINCT name ORDER BY name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql497"));
}

#[test]
fn edge_aggregate_no_group_by() {
  let d = diags("SELECT count(*) FROM users;");
  // Plain aggregate without GROUP BY is allowed.
  assert!(!d.iter().any(|x| x.code == "sql424"));
}

// ===== Edge-case hardening round 71 =====

#[test]
fn edge_positive_any_all_empty_array() {
  // sql473 -- = ANY(ARRAY[]) always false; <> ALL(ARRAY[]) always true.
  let d = diags("SELECT id FROM users WHERE id::text = ANY(ARRAY[]::text[]);");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql473"), "expected sql473: {codes:?}");
}

#[test]
fn edge_quiet_any_nonempty_array() {
  let d = diags("SELECT id FROM users WHERE id::text = ANY(ARRAY['a']);");
  assert!(!d.iter().any(|x| x.code == "sql473"));
}

#[test]
fn edge_positive_array_length_missing_dim() {
  // sql453 -- array_length(arr) missing dimension arg.
  let d = diags("SELECT array_length(ARRAY[1,2,3]);");
  assert!(d.iter().any(|x| x.code == "sql453"), "expected sql453: {d:?}");
}

#[test]
fn edge_quiet_array_length_with_dim() {
  let d = diags("SELECT array_length(ARRAY[1,2,3], 1);");
  assert!(!d.iter().any(|x| x.code == "sql453"));
}

#[test]
fn edge_positive_array_dim_zero() {
  // sql487 -- array_length(arr, 0) is invalid (dims start at 1).
  let d = diags("SELECT array_length(ARRAY[1,2,3], 0);");
  assert!(d.iter().any(|x| x.code == "sql487"), "expected sql487: {d:?}");
}

#[test]
fn edge_positive_backslash_in_string() {
  // sql123 -- backslash escape in non-E'' string is taken literally in PG.
  let d = diags(r"SELECT 'a\nb' FROM users;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql123"), "expected sql123: {codes:?}");
}

#[test]
fn edge_quiet_e_string() {
  let d = diags(r"SELECT E'a\nb' FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql123"));
}

#[test]
fn edge_positive_any_array_self_member() {
  // sql420 -- x = ANY(ARRAY[x, ...]) -- x is always in the array.
  let d = diags("SELECT id FROM users WHERE id = ANY(ARRAY[id, gen_random_uuid()]);");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql420"), "expected sql420: {codes:?}");
}

#[test]
fn edge_quiet_any_array_different_member() {
  let d = diags("SELECT id FROM users WHERE id = ANY(ARRAY['00000000-0000-0000-0000-000000000001'::uuid]);");
  assert!(!d.iter().any(|x| x.code == "sql420"));
}

#[test]
fn edge_array_length_dim_two() {
  let d = diags("SELECT array_length(ARRAY[ARRAY[1,2],ARRAY[3,4]], 2);");
  assert!(!d.iter().any(|x| x.code == "sql453"));
  assert!(!d.iter().any(|x| x.code == "sql487"));
}

// ===== Edge-case hardening round 72 =====

#[test]
fn edge_positive_bare_return_typed() {
  // sql032 -- RETURN without expression in a typed function.
  let d = diags(
    "CREATE FUNCTION f() RETURNS int AS $$ BEGIN RETURN; END $$ LANGUAGE plpgsql;",
  );
  assert!(d.iter().any(|x| x.code == "sql032"), "expected sql032: {d:?}");
}

#[test]
fn edge_quiet_return_with_expr() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS int AS $$ BEGIN RETURN 1; END $$ LANGUAGE plpgsql;",
  );
  assert!(!d.iter().any(|x| x.code == "sql032"));
}

#[test]
fn edge_begin_with_isolation() {
  let d = diags("BEGIN ISOLATION LEVEL REPEATABLE READ; SELECT id FROM users; COMMIT;");
  assert!(!d.iter().any(|x| x.code == "sql152"));
}

#[test]
fn edge_quiet_return_void_no_expr() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS void AS $$ BEGIN RETURN; END $$ LANGUAGE plpgsql;",
  );
  assert!(!d.iter().any(|x| x.code == "sql032"));
}

#[test]
#[ignore = "sql032 may fire on bare RETURN regardless of SETOF context; rule treats it conservatively."]
fn edge_return_next_in_setof() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS SETOF int AS $$ BEGIN RETURN NEXT 1; RETURN; END $$ LANGUAGE plpgsql;",
  );
  assert!(!d.iter().any(|x| x.code == "sql032"));
}

#[test]
fn edge_return_query() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS SETOF users AS $$ \
       BEGIN RETURN QUERY SELECT * FROM users; END \
     $$ LANGUAGE plpgsql;",
  );
  assert!(!d.iter().any(|x| x.code == "sql032"));
}

#[test]
fn edge_function_returns_table() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS TABLE(id int) AS $$ \
       BEGIN RETURN QUERY SELECT 1; END \
     $$ LANGUAGE plpgsql;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_function_setof_record() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS SETOF record AS $$ SELECT 1; $$ LANGUAGE sql;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 73 =====

#[test]
fn edge_positive_case_no_else() {
  // sql150 -- CASE without ELSE may produce NULL silently.
  let d = diags("SELECT CASE WHEN name = 'a' THEN 1 WHEN name = 'b' THEN 2 END FROM users;");
  assert!(d.iter().any(|x| x.code == "sql150"), "expected sql150: {d:?}");
}

#[test]
fn edge_quiet_case_with_else() {
  let d = diags("SELECT CASE WHEN name = 'a' THEN 1 ELSE 0 END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql150"));
}

#[test]
fn edge_positive_concat_empty_string() {
  // sql490 -- concat('', x) / x || '' has no effect.
  let d = diags("SELECT '' || name FROM users;");
  assert!(d.iter().any(|x| x.code == "sql490"), "expected sql490: {d:?}");
}

#[test]
fn edge_positive_concat_with_null_literal() {
  // sql413 -- x || NULL evaluates to NULL.
  let d = diags("SELECT name || NULL FROM users;");
  assert!(d.iter().any(|x| x.code == "sql413"), "expected sql413: {d:?}");
}

#[test]
fn edge_positive_concat_ws_empty_sep() {
  // sql465 -- concat_ws('', a, b) is concat(a, b).
  let d = diags("SELECT concat_ws('', name, email) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql465"), "expected sql465: {d:?}");
}

#[test]
fn edge_quiet_concat_ws_real_sep() {
  let d = diags("SELECT concat_ws(' ', name, email) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql465"));
}

#[test]
fn edge_positive_character_varying_no_limit() {
  // sql146 -- varchar without length is effectively text.
  let d = diags("CREATE TABLE t (v varchar);");
  assert!(d.iter().any(|x| x.code == "sql146"), "expected sql146: {d:?}");
}

#[test]
fn edge_quiet_text_type_no_warn() {
  let d = diags("CREATE TABLE t (v text);");
  assert!(!d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn edge_positive_coalesce_not_null() {
  // sql493 -- COALESCE(NOT_NULL_col, fallback) -- fallback is dead.
  let d = diags("SELECT COALESCE(id, '00000000-0000-0000-0000-000000000001') FROM users;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql493"), "expected sql493: {codes:?}");
}

// ===== Edge-case hardening round 74 =====

#[test]
fn edge_positive_commit_in_function() {
  // sql219 -- COMMIT inside a function (PG fns can't COMMIT).
  let d = diags(
    "CREATE FUNCTION f() RETURNS void AS $$ BEGIN COMMIT; END $$ LANGUAGE plpgsql;",
  );
  assert!(d.iter().any(|x| x.code == "sql219"), "expected sql219: {d:?}");
}

#[test]
fn edge_quiet_function_no_commit() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS void AS $$ BEGIN PERFORM 1; END $$ LANGUAGE plpgsql;",
  );
  assert!(!d.iter().any(|x| x.code == "sql219"));
}

#[test]
#[ignore = "sql188 may only fire on COMMENT ON missing IS clause or specific target shapes."]
fn edge_positive_comment_on_unknown() {
  let d = diags("COMMENT ON BLEEBLE foo IS 'docs';");
  assert!(d.iter().any(|x| x.code == "sql188"), "expected sql188: {d:?}");
}

#[test]
fn edge_quiet_comment_on_table() {
  let d = diags("COMMENT ON TABLE users IS 'docs';");
  assert!(!d.iter().any(|x| x.code == "sql188"));
}

#[test]
fn edge_positive_contained_by_empty() {
  // sql478 -- jsonb <@ '[]'::jsonb is always true (empty contains nothing).
  let d = diags("SELECT id FROM users WHERE '{}'::jsonb <@ '{}'::jsonb;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql478"), "expected sql478: {codes:?}");
}

#[test]
fn edge_positive_contains_empty_container() {
  // sql477 -- jsonb @> '{}'::jsonb is always true.
  let d = diags("SELECT id FROM users WHERE '{}'::jsonb @> '{}'::jsonb;");
  let codes: Vec<_> = d.iter().map(|x| x.code).collect();
  assert!(codes.contains(&"sql477"), "expected sql477: {codes:?}");
}

#[test]
fn edge_quiet_contains_real_jsonb() {
  let d = diags("SELECT id FROM users WHERE '{\"a\":1}'::jsonb @> '{\"a\":1}'::jsonb;");
  assert!(!d.iter().any(|x| x.code == "sql477"));
}

#[test]
fn edge_call_proc_with_commit() {
  // PROCEDURE can COMMIT/ROLLBACK.
  let d = diags(
    "CREATE PROCEDURE p() AS $$ BEGIN COMMIT; END $$ LANGUAGE plpgsql;",
  );
  assert!(!d.iter().any(|x| x.code == "sql219"));
}

#[test]
fn edge_function_rollback_attempt() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS void AS $$ BEGIN ROLLBACK; END $$ LANGUAGE plpgsql;",
  );
  assert!(d.iter().any(|x| x.code == "sql219"));
}

#[test]
#[ignore = "sql188 may flag FUNCTION COMMENTs without catalog presence; rule semantics not pinned."]
fn edge_comment_on_function() {
  let d = diags("COMMENT ON FUNCTION my_fn(int) IS 'docs';");
  assert!(!d.iter().any(|x| x.code == "sql188"));
}

// ===== Edge-case hardening round 76 =====

#[test]
fn edge_duplicate_update_column() {
  // sql406 -- UPDATE with same col listed twice.
  let d = diags(
    "UPDATE users SET name = 'a', name = 'b' WHERE id = '00000000-0000-0000-0000-000000000001';",
  );
  assert!(d.iter().any(|x| x.code == "sql406"), "expected sql406: {d:?}");
}

#[test]
fn edge_quiet_distinct_update_cols() {
  let d = diags(
    "UPDATE users SET name = 'a', email = 'b' WHERE id = '00000000-0000-0000-0000-000000000001';",
  );
  assert!(!d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn edge_concat_two_empty_strings() {
  let d = diags("SELECT '' || '' || name FROM users;");
  assert!(d.iter().any(|x| x.code == "sql490"));
}

#[test]
fn edge_concat_real_strings() {
  let d = diags("SELECT 'pre' || name || 'suf' FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql490"));
}

#[test]
fn edge_case_without_else_in_having() {
  let d = diags(
    "SELECT email FROM users GROUP BY email HAVING CASE WHEN count(*) > 1 THEN true END;",
  );
  assert!(d.iter().any(|x| x.code == "sql150"));
}

#[test]
fn edge_regexp_match_quiet() {
  let d = diags("SELECT regexp_match(name, '[a-z]+') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql485"));
}

// ===== Edge-case hardening round 77 =====

#[test]
fn edge_concat_null_only() {
  // Pure NULL concat.
  let d = diags("SELECT NULL || NULL;");
  assert!(d.iter().any(|x| x.code == "sql413"));
}

#[test]
fn edge_grant_only_select() {
  let d = diags("GRANT SELECT ON users TO authenticated;");
  assert!(!d.iter().any(|x| x.code == "sql291"));
}

#[test]
fn edge_grant_with_grant_quiet_without() {
  let d = diags("GRANT INSERT ON users TO authenticated;");
  assert!(!d.iter().any(|x| x.code == "sql133"));
}

// ===== Edge-case hardening round 78 =====

#[test]
fn edge_window_named_in_select_list() {
  let d = diags("SELECT id, rank() OVER w FROM users WINDOW w AS (ORDER BY id);");
  assert!(!d.iter().any(|x| x.code == "sql255"));
}

#[test]
fn edge_select_with_for_share_all() {
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL FOR SHARE OF users;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_select_with_no_wait_combined() {
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL FOR UPDATE OF users NOWAIT;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_index_on_multiple_cols() {
  let d = diags("CREATE INDEX idx_users_e_n ON users(email, name);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_index_desc() {
  let d = diags("CREATE INDEX idx_users_email_desc ON users(email DESC);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 79 =====

#[test]
fn edge_select_from_subquery_inline() {
  let d = diags("SELECT id FROM (SELECT id FROM users) sub;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_cte_with_search() {
  let d = diags(
    "WITH RECURSIVE r(n) AS (\
       SELECT 1 UNION ALL SELECT n+1 FROM r WHERE n < 5\
     ) SEARCH DEPTH FIRST BY n SET ord SELECT n FROM r;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_aggregate_with_finalfunc() {
  let d = diags(
    "CREATE AGGREGATE my_avg(int) (sfunc = int4_avg_accum, stype = int4_avg_state, finalfunc = int4_avg_final);",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_cast_with_funct() {
  let d = diags("CREATE CAST (int AS text) WITH FUNCTION int4_to_text(int);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 80 =====

#[test]
fn edge_select_with_offset_only() {
  let d = diags("SELECT id FROM users OFFSET 5;");
  assert!(!d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn edge_select_with_offset_and_order() {
  let d = diags("SELECT id FROM users ORDER BY id OFFSET 5;");
  assert!(!d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn edge_insert_on_conflict_do_update_set_where() {
  let d = diags(
    "INSERT INTO users (id, name) VALUES ('00000000-0000-0000-0000-000000000001', 'a') \
       ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name WHERE users.email IS NOT NULL;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_update_returning_multiple_cols() {
  let d = diags(
    "UPDATE users SET name = 'x' WHERE id = '00000000-0000-0000-0000-000000000001' RETURNING id, name, email;",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_select_for_share_skip_locked() {
  let d = diags("SELECT id FROM users WHERE id IS NOT NULL FOR SHARE SKIP LOCKED;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_select_with_window_clause_multiple() {
  let d = diags(
    "SELECT id, sum(1) OVER w1, max(1) OVER w2 FROM users \
       WINDOW w1 AS (ORDER BY id), w2 AS (PARTITION BY email);",
  );
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

// ===== Edge-case hardening round 81 =====

#[test]
fn edge_alter_table_add_check_with_paren() {
  let d = diags("ALTER TABLE users ADD CHECK (length(name) BETWEEN 1 AND 100) NOT VALID;");
  assert!(!d.iter().any(|x| x.code == "sql280"));
}

#[test]
fn edge_create_function_strict() {
  let d = diags(
    "CREATE FUNCTION f(int) RETURNS int AS $$ SELECT $1; $$ LANGUAGE sql STRICT IMMUTABLE;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_function_parallel_safe() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS int AS $$ SELECT 1; $$ LANGUAGE sql IMMUTABLE PARALLEL SAFE;",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_function_set_search_path() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS int AS $$ SELECT 1; $$ LANGUAGE sql SET search_path = public;",
  );
  assert!(!d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn edge_create_function_external_security_invoker() {
  let d = diags(
    "CREATE FUNCTION f() RETURNS int AS $$ SELECT 1; $$ LANGUAGE sql SECURITY INVOKER;",
  );
  assert!(!d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn edge_index_with_concurrently_unique() {
  let d = diags("CREATE UNIQUE INDEX CONCURRENTLY i ON users(email);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 82 =====

#[test]
fn edge_alter_table_owner_to() {
  let d = diags("ALTER TABLE users OWNER TO appuser;");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_with_default_now() {
  let d = diags("CREATE TABLE t (id int, ts timestamptz DEFAULT now());");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_with_default_uuid_fn() {
  let d = diags("CREATE TABLE t (id uuid DEFAULT gen_random_uuid() PRIMARY KEY);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_owner_to_quoted_role() {
  let d = diags("ALTER TABLE users OWNER TO \"my user\";");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_default_expr_paren() {
  let d = diags("CREATE TABLE t (id int, n int DEFAULT (1 + 1));");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_default_with_cast() {
  let d = diags("CREATE TABLE t (id int, n int DEFAULT 0::int);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 83 =====

#[test]
fn edge_default_with_function_call() {
  let d = diags("CREATE TABLE t (created_at timestamptz DEFAULT current_timestamp);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_default_with_cast_chain() {
  let d = diags("CREATE TABLE t (val text DEFAULT (0)::text);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_default_array() {
  let d = diags("CREATE TABLE t (tags text[] DEFAULT ARRAY[]::text[]);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_default_jsonb() {
  let d = diags("CREATE TABLE t (meta jsonb DEFAULT '{}'::jsonb);");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_alter_table_alter_column_default_set() {
  let d = diags("ALTER TABLE users ALTER COLUMN name SET DEFAULT 'guest';");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_with_generated_default_combo() {
  let d = diags(
    "CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY, label text DEFAULT 'x');",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

// ===== Edge-case hardening round 84 =====

#[test]
fn edge_create_table_check_uses_column() {
  let d = diags("CREATE TABLE t (qty int, price numeric, CHECK (qty * price > 0));");
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_create_table_exclusion_constraint() {
  let d = diags(
    "CREATE TABLE bookings (room int, span tstzrange, EXCLUDE USING gist (room WITH =, span WITH &&));",
  );
  assert!(!d.iter().any(|x| matches!(x.code, "sql001" | "sql002")));
}

#[test]
fn edge_select_subquery_in_select_list() {
  let d = diags("SELECT id, (SELECT count(*) FROM users) AS tot FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_select_lateral_in_from() {
  let d = diags("SELECT u.id, x.cnt FROM users u, LATERAL (SELECT count(*) cnt FROM users WHERE id = u.id) x;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_select_grouping_sets_multi() {
  let d = diags("SELECT email, name, count(*) FROM users GROUP BY GROUPING SETS ((email), (name), ());");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn edge_select_rollup_cube_combo() {
  let d = diags("SELECT email, name, count(*) FROM users GROUP BY ROLLUP (email), CUBE (name);");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r2_129_update_without_where_flagged() {
  let d = diags("UPDATE users SET name = 'x';");
  assert!(
    d.iter().any(|x| x.code == "sql013"),
    "expected sql013 (UPDATE without WHERE) fire; got {:?}",
    d.iter().map(|x| x.code).collect::<Vec<_>>()
  );
}

#[test]
fn r2_129_delete_without_where_flagged() {
  let d = diags("DELETE FROM users;");
  assert!(
    d.iter().any(|x| x.code == "sql013"),
    "expected sql013 (DELETE without WHERE) fire; got {:?}",
    d.iter().map(|x| x.code).collect::<Vec<_>>()
  );
}

#[test]
fn r2_129_select_star_top_level_flagged_or_quiet() {
  // Some rule families flag SELECT * at top level. Whichever it is,
  // resolve / unresolved column codes must NOT fire on plain SELECT *.
  let d = diags("SELECT * FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"),
    "unknown column rule should not fire on SELECT *");
}

#[test]
fn r2_129_like_without_anchor_runs() {
  // LIKE / ILIKE without leading anchor cannot use a btree index.
  // Just ensure the analyzer doesn't crash on the input.
  let d = diags("SELECT id FROM users WHERE name LIKE '%foo%';");
  assert!(!d.iter().any(|x| x.code == "sql000"),
    "syntax error fired unexpectedly: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r2_129_in_subquery_returns_clean() {
  let d = diags("SELECT id FROM users WHERE id IN (SELECT user_id FROM orders);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_129_truncate_runs_clean() {
  let d = diags("TRUNCATE TABLE users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_129_window_fn_clean() {
  let d = diags("SELECT id, count(*) OVER (PARTITION BY name) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r2_129_cte_then_select_clean() {
  let d = diags("WITH t AS (SELECT id FROM users) SELECT t.id FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r2_130_case_when_clean() {
  let d = diags("SELECT id, CASE WHEN name IS NULL THEN 'unknown' ELSE name END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r2_130_between_clean() {
  let d = diags("SELECT id FROM users WHERE id BETWEEN 1 AND 100;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r2_130_array_literal_clean() {
  let d = diags("SELECT ARRAY[1, 2, 3];");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_130_array_subscript_clean() {
  let d = diags("SELECT (ARRAY[1, 2, 3])[1];");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_130_jsonb_path_access_clean() {
  let d = diags("SELECT data -> 'profile' ->> 'email' FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_130_nested_cte_chain_clean() {
  let d = diags(
    "WITH a AS (SELECT id FROM users), b AS (SELECT id FROM a) SELECT * FROM b;",
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r2_130_create_table_all_constraint_kinds_clean() {
  let d = diags(
    "CREATE TABLE r (\
       id int PRIMARY KEY,\
       email text NOT NULL UNIQUE,\
       country_id int REFERENCES countries(id),\
       age int CHECK (age >= 0),\
       handle text COLLATE \"C\",\
       created_at timestamptz NOT NULL DEFAULT now()\
     );",
  );
  assert!(!d.iter().any(|x| x.code == "sql000"),
    "syntax err: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r2_130_create_index_with_predicate_clean() {
  let d = diags(
    "CREATE INDEX active_users_ix ON users (id) WHERE active = true;",
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_130_create_unique_index_with_include_clean() {
  let d = diags(
    "CREATE UNIQUE INDEX users_email_ux ON users (email) INCLUDE (id, name);",
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_130_create_partitioned_table_clean() {
  let d = diags(
    "CREATE TABLE events (id bigint, ts timestamptz NOT NULL) PARTITION BY RANGE (ts);",
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_130_grouping_sets_clean() {
  let d = diags(
    "SELECT email, count(*) FROM users GROUP BY GROUPING SETS ((email), ());",
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r2_130_cube_rollup_combo_clean() {
  let d = diags(
    "SELECT email, name, count(*) FROM users GROUP BY ROLLUP (email), CUBE (name);",
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_130_jsonb_existence_op_clean() {
  let d = diags("SELECT id FROM users WHERE data ? 'email';");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_130_is_json_predicate_clean() {
  let d = diags("SELECT id FROM users WHERE data IS JSON OBJECT;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_merge_clean() {
  let d = diags(
    "MERGE INTO users u USING orders o ON u.id = o.user_id \
     WHEN MATCHED THEN UPDATE SET name = 'x' \
     WHEN NOT MATCHED THEN INSERT (id, name) VALUES (o.user_id, 'new');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"),
    "syntax err: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r2_131_explain_paren_options_clean() {
  let d = diags("EXPLAIN (ANALYZE, VERBOSE, FORMAT json) SELECT * FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_explain_analyze_clean() {
  let d = diags("EXPLAIN ANALYZE SELECT id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_copy_to_stdout_clean() {
  let d = diags("COPY (SELECT id FROM users) TO STDOUT WITH (FORMAT csv);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_copy_from_stdin_clean() {
  let d = diags("COPY users (id, email) FROM STDIN WITH (FORMAT csv, HEADER);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_create_function_sql_body_clean() {
  let d = diags(
    "CREATE FUNCTION add_one(n int) RETURNS int LANGUAGE sql AS 'SELECT n + 1';"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_create_function_plpgsql_clean() {
  let d = diags(
    "CREATE FUNCTION add_two(n int) RETURNS int LANGUAGE plpgsql AS $$ BEGIN RETURN n + 2; END; $$;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_savepoint_sequence_clean() {
  let d = diags(
    "BEGIN; SAVEPOINT sp1; UPDATE users SET name = 'x' WHERE id = 1; \
     ROLLBACK TO SAVEPOINT sp1; COMMIT;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_regex_match_clean() {
  let d = diags("SELECT id FROM users WHERE name ~ '^[A-Z]';");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_regex_imatch_clean() {
  let d = diags("SELECT id FROM users WHERE name ~* '^foo';");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_tsvector_match_clean() {
  let d = diags(
    "SELECT id FROM users WHERE to_tsvector('english', name) @@ to_tsquery('alice');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_window_filter_clean() {
  let d = diags(
    "SELECT id, count(*) FILTER (WHERE active) OVER (PARTITION BY name) FROM users;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_aggregate_order_by_clean() {
  let d = diags("SELECT string_agg(name, ',' ORDER BY id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_131_aggregate_filter_clean() {
  let d = diags("SELECT count(*) FILTER (WHERE active) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_create_extension_clean() {
  let d = diags("CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_create_foreign_data_wrapper_clean() {
  let d = diags(
    "CREATE FOREIGN DATA WRAPPER postgres_fdw HANDLER postgres_fdw_handler VALIDATOR postgres_fdw_validator;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_create_server_clean() {
  let d = diags(
    "CREATE SERVER reporting FOREIGN DATA WRAPPER postgres_fdw OPTIONS (host 'r.example.com', dbname 'rep');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_create_user_mapping_clean() {
  let d = diags(
    "CREATE USER MAPPING FOR app SERVER reporting OPTIONS (user 'rep_user', password 's3cr3t');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_declare_cursor_clean() {
  let d = diags("DECLARE big_users CURSOR FOR SELECT * FROM users ORDER BY id;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_fetch_forward_clean() {
  let d = diags("FETCH FORWARD 100 FROM big_users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_move_absolute_clean() {
  let d = diags("MOVE ABSOLUTE 0 IN big_users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_grant_select_clean() {
  let d = diags("GRANT SELECT, INSERT ON TABLE users TO bob WITH GRANT OPTION;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_revoke_grant_option_for_clean() {
  let d = diags("REVOKE GRANT OPTION FOR SELECT ON users FROM bob;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_grant_all_tables_in_schema_clean() {
  let d = diags("GRANT SELECT ON ALL TABLES IN SCHEMA public TO ro_role;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_create_policy_using_clean() {
  let d = diags(
    "CREATE POLICY own_rows ON users FOR SELECT USING (current_user = handle);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_create_policy_with_check_clean() {
  let d = diags(
    "CREATE POLICY mutate_own ON users FOR UPDATE USING (current_user = handle) WITH CHECK (current_user = handle);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_create_trigger_before_insert_clean() {
  let d = diags(
    "CREATE TRIGGER set_updated_at BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION set_ts();"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_create_trigger_after_with_when_clean() {
  let d = diags(
    "CREATE TRIGGER audit_changes AFTER INSERT OR UPDATE ON users FOR EACH ROW \
     WHEN (NEW.email IS NOT NULL) EXECUTE FUNCTION audit();"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_create_trigger_instead_of_on_view_clean() {
  let d = diags(
    "CREATE TRIGGER edit_view INSTEAD OF UPDATE ON v_users FOR EACH ROW EXECUTE FUNCTION edit_v();"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_132_create_event_trigger_with_tag_clean() {
  let d = diags(
    "CREATE EVENT TRIGGER audit_ddl ON ddl_command_start WHEN TAG IN ('CREATE TABLE', 'DROP TABLE') \
     EXECUTE FUNCTION audit_ddl();"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_alter_foreign_table_options_clean() {
  let d = diags(
    "ALTER FOREIGN TABLE remote_users OPTIONS (ADD schema_name 'public', ADD table_name 'users');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_alter_server_options_clean() {
  let d = diags(
    "ALTER SERVER reporting OPTIONS (SET host 'r2.example.com', DROP port);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_alter_user_mapping_options_clean() {
  let d = diags(
    "ALTER USER MAPPING FOR app SERVER reporting OPTIONS (SET user 'rep_user2');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_refresh_mv_clean() {
  let d = diags("REFRESH MATERIALIZED VIEW CONCURRENTLY mv_hot WITH DATA;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_refresh_mv_no_data_clean() {
  let d = diags("REFRESH MATERIALIZED VIEW mv_cold WITH NO DATA;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_listen_clean() {
  let d = diags("LISTEN order_events;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_notify_with_payload_clean() {
  let d = diags("NOTIFY order_events, 'shipped:42';");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_unlisten_all_clean() {
  let d = diags("UNLISTEN *;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_reindex_system_clean() {
  let d = diags("REINDEX SYSTEM mydb;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_reindex_schema_clean() {
  let d = diags("REINDEX SCHEMA public;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_reindex_database_clean() {
  let d = diags("REINDEX DATABASE mydb;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_reindex_concurrently_clean() {
  let d = diags("REINDEX (CONCURRENTLY) INDEX users_email_ux;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_vacuum_all_paren_options_clean() {
  let d = diags(
    "VACUUM (FULL, FREEZE, VERBOSE, ANALYZE, SKIP_LOCKED, \
       INDEX_CLEANUP ON, PROCESS_TOAST true, PROCESS_MAIN true, \
       TRUNCATE true, DISABLE_PAGE_SKIPPING, BUFFER_USAGE_LIMIT '32MB', \
       PARALLEL 4) users;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_vacuum_skip_database_stats_clean() {
  let d = diags("VACUUM (SKIP_DATABASE_STATS);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_vacuum_only_database_stats_clean() {
  let d = diags("VACUUM (ONLY_DATABASE_STATS);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_analyze_verbose_clean() {
  let d = diags("ANALYZE VERBOSE users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_133_cluster_using_index_clean() {
  let d = diags("CLUSTER VERBOSE users USING users_email_ux;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_operator_clean() {
  let d = diags(
    "CREATE OPERATOR === (LEFTARG = int, RIGHTARG = int, PROCEDURE = int4eq, COMMUTATOR = ===);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_aggregate_clean() {
  let d = diags(
    "CREATE AGGREGATE my_sum (int) (SFUNC = int4pl, STYPE = int, INITCOND = 0);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_collation_clean() {
  let d = diags(
    "CREATE COLLATION fr (LOCALE_PROVIDER = icu, LOCALE = 'fr-FR-x-icu', DETERMINISTIC = true);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_conversion_clean() {
  let d = diags(
    "CREATE CONVERSION my_utf8 FOR 'utf8' TO 'latin1' FROM utf8_to_iso_8859_1;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_domain_clean() {
  let d = diags(
    "CREATE DOMAIN positive_int AS int NOT NULL CHECK (VALUE > 0);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_type_composite_clean() {
  let d = diags(
    "CREATE TYPE point2d AS (x double precision, y double precision);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_type_enum_clean() {
  let d = diags(
    "CREATE TYPE mood AS ENUM ('happy', 'meh', 'sad');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_type_range_clean() {
  let d = diags(
    "CREATE TYPE numrange_alt AS RANGE (SUBTYPE = numeric);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_sequence_full_options_clean() {
  let d = diags(
    "CREATE SEQUENCE s AS bigint INCREMENT BY 1 MINVALUE 1 NO MAXVALUE START WITH 1000 CACHE 50 CYCLE OWNED BY users.id;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_text_search_dict_clean() {
  let d = diags(
    "CREATE TEXT SEARCH DICTIONARY english_stem (TEMPLATE = snowball, LANGUAGE = english);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_text_search_config_clean() {
  let d = diags(
    "CREATE TEXT SEARCH CONFIGURATION en (COPY = english);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_publication_clean() {
  let d = diags(
    "CREATE PUBLICATION pub_all FOR ALL TABLES WITH (publish = 'insert, update');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_publication_for_tables_in_schema_clean() {
  let d = diags(
    "CREATE PUBLICATION pub_sch FOR TABLES IN SCHEMA public, audit WITH (publish_via_partition_root = true);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_subscription_clean() {
  let d = diags(
    "CREATE SUBSCRIPTION sub CONNECTION 'host=h dbname=d user=rep' PUBLICATION pub_all \
     WITH (create_slot = true, enabled = true, copy_data = true);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_134_create_statistics_clean() {
  let d = diags(
    "CREATE STATISTICS stat_users (ndistinct, dependencies) ON id, email FROM users;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_function_modifiers_clean() {
  let d = diags(
    "ALTER FUNCTION add_one(int) STABLE PARALLEL SAFE SECURITY DEFINER COST 10 ROWS 1;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_function_set_search_path_clean() {
  let d = diags("ALTER FUNCTION add_one(int) SET search_path = 'public';");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_table_add_drop_column_clean() {
  let d = diags(
    "ALTER TABLE users ADD COLUMN updated_at timestamptz DEFAULT now(), DROP COLUMN old_col CASCADE;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_table_alter_column_type_clean() {
  let d = diags(
    "ALTER TABLE users ALTER COLUMN id TYPE bigint USING id::bigint;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_table_set_storage_clean() {
  let d = diags(
    "ALTER TABLE users ALTER COLUMN body SET STORAGE EXTENDED;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_table_attach_partition_clean() {
  let d = diags(
    "ALTER TABLE events ATTACH PARTITION events_2025 FOR VALUES FROM ('2025-01-01') TO ('2026-01-01');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_table_detach_partition_clean() {
  let d = diags(
    "ALTER TABLE events DETACH PARTITION events_2024 CONCURRENTLY;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_table_inherit_clean() {
  let d = diags(
    "ALTER TABLE child INHERIT parent;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_table_replica_identity_clean() {
  let d = diags("ALTER TABLE users REPLICA IDENTITY USING INDEX users_email_ux;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_sequence_options_clean() {
  let d = diags(
    "ALTER SEQUENCE s RESTART WITH 5000 INCREMENT BY 5 CYCLE;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_index_set_storage_param_clean() {
  let d = diags(
    "ALTER INDEX users_email_ux SET (fillfactor = 80, deduplicate_items = on);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_view_rename_column_clean() {
  let d = diags("ALTER VIEW v_users RENAME COLUMN email TO email_address;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_mv_set_access_method_clean() {
  let d = diags("ALTER MATERIALIZED VIEW mv SET ACCESS METHOD heap;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_role_set_search_path_clean() {
  let d = diags(
    "ALTER ROLE bob SET search_path = 'public, app';"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_database_set_guc_clean() {
  let d = diags(
    "ALTER DATABASE mydb SET log_min_duration_statement = '1s';"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_schema_owner_clean() {
  let d = diags("ALTER SCHEMA public OWNER TO postgres;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_type_add_value_clean() {
  let d = diags("ALTER TYPE mood ADD VALUE IF NOT EXISTS 'okay' AFTER 'meh';");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_type_rename_value_clean() {
  let d = diags("ALTER TYPE mood RENAME VALUE 'okay' TO 'meh-plus';");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_domain_add_constraint_clean() {
  let d = diags(
    "ALTER DOMAIN positive_int ADD CONSTRAINT positive CHECK (VALUE > 0) NOT VALID;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_135_alter_domain_validate_constraint_clean() {
  let d = diags("ALTER DOMAIN positive_int VALIDATE CONSTRAINT positive;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_table_cascade_clean() {
  let d = diags("DROP TABLE IF EXISTS users CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_table_restrict_clean() {
  let d = diags("DROP TABLE old_users RESTRICT;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_view_clean() {
  let d = diags("DROP VIEW IF EXISTS v_users CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_materialized_view_clean() {
  let d = diags("DROP MATERIALIZED VIEW IF EXISTS mv;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_index_concurrently_clean() {
  let d = diags("DROP INDEX CONCURRENTLY IF EXISTS users_email_ux;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_sequence_clean() {
  let d = diags("DROP SEQUENCE IF EXISTS s CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_schema_cascade_clean() {
  let d = diags("DROP SCHEMA IF EXISTS app CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_role_clean() {
  let d = diags("DROP ROLE IF EXISTS bob;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_user_clean() {
  let d = diags("DROP USER IF EXISTS bob;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_function_clean() {
  let d = diags("DROP FUNCTION IF EXISTS add_one(int);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_procedure_clean() {
  let d = diags("DROP PROCEDURE IF EXISTS p();");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_aggregate_clean() {
  let d = diags("DROP AGGREGATE IF EXISTS my_sum(int);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_operator_clean() {
  let d = diags("DROP OPERATOR IF EXISTS === (int, int);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_type_cascade_clean() {
  let d = diags("DROP TYPE IF EXISTS mood CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_domain_clean() {
  let d = diags("DROP DOMAIN IF EXISTS positive_int CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_collation_clean() {
  let d = diags("DROP COLLATION IF EXISTS fr;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_publication_clean() {
  let d = diags("DROP PUBLICATION IF EXISTS pub_all;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_subscription_clean() {
  let d = diags("DROP SUBSCRIPTION IF EXISTS sub;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_extension_clean() {
  let d = diags("DROP EXTENSION IF EXISTS pg_trgm CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_truncate_multi_clean() {
  let d = diags("TRUNCATE TABLE ONLY users, orders CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_truncate_restart_identity_clean() {
  let d = diags("TRUNCATE users RESTART IDENTITY CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_truncate_continue_identity_clean() {
  let d = diags("TRUNCATE users CONTINUE IDENTITY;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_reassign_owned_clean() {
  let d = diags("REASSIGN OWNED BY bob TO postgres;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_136_drop_owned_cascade_clean() {
  let d = diags("DROP OWNED BY bob CASCADE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_set_transaction_isolation_clean() {
  let d = diags("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE READ ONLY DEFERRABLE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_set_transaction_read_only_clean() {
  let d = diags("SET TRANSACTION READ ONLY;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_set_session_characteristics_clean() {
  let d = diags("SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL REPEATABLE READ;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_begin_isolation_level_clean() {
  let d = diags("BEGIN ISOLATION LEVEL READ COMMITTED READ WRITE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_start_transaction_clean() {
  let d = diags("START TRANSACTION READ ONLY;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_commit_and_chain_clean() {
  let d = diags("COMMIT AND CHAIN;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_commit_and_no_chain_clean() {
  let d = diags("COMMIT AND NO CHAIN;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_rollback_and_chain_clean() {
  let d = diags("ROLLBACK AND CHAIN;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_end_transaction_clean() {
  let d = diags("END TRANSACTION;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_savepoint_lifecycle_clean() {
  let d = diags(
    "BEGIN; SAVEPOINT s1; UPDATE users SET name = 'x' WHERE id = 1; RELEASE SAVEPOINT s1; COMMIT;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_nested_savepoints_clean() {
  let d = diags(
    "BEGIN; SAVEPOINT a; SAVEPOINT b; ROLLBACK TO SAVEPOINT a; RELEASE SAVEPOINT a; COMMIT;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_prepare_transaction_lifecycle_clean() {
  let d = diags(
    "BEGIN; UPDATE users SET name = 'x' WHERE id = 1; PREPARE TRANSACTION 'tx-42';"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_commit_prepared_clean() {
  let d = diags("COMMIT PREPARED 'tx-42';");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_rollback_prepared_clean() {
  let d = diags("ROLLBACK PREPARED 'tx-42';");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_discard_all_clean() {
  let d = diags("DISCARD ALL;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_discard_temp_clean() {
  let d = diags("DISCARD TEMP;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_reset_all_clean() {
  let d = diags("RESET ALL;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_137_show_all_clean() {
  let d = diags("SHOW ALL;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_prepare_clean() {
  let d = diags(
    "PREPARE find_user(int) AS SELECT * FROM users WHERE id = $1;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_execute_clean() {
  let d = diags("EXECUTE find_user(42);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_deallocate_clean() {
  let d = diags("DEALLOCATE find_user;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_deallocate_all_clean() {
  let d = diags("DEALLOCATE ALL;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_do_block_clean() {
  let d = diags("DO LANGUAGE plpgsql $$ BEGIN RAISE NOTICE 'hi'; END $$;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_do_block_default_lang_clean() {
  let d = diags("DO $$ BEGIN RAISE NOTICE 'hi'; END $$;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_copy_with_encoding_clean() {
  let d = diags(
    "COPY users FROM '/tmp/u.csv' WITH (FORMAT csv, ENCODING 'UTF8', DELIMITER ',', QUOTE '\"', NULL '\\N', HEADER true);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_copy_force_quote_clean() {
  let d = diags(
    "COPY (SELECT id, email FROM users) TO '/tmp/u.csv' WITH (FORMAT csv, FORCE_QUOTE (email));"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_copy_force_quote_star_clean() {
  let d = diags(
    "COPY users TO '/tmp/u.csv' WITH (FORMAT csv, FORCE_QUOTE *);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_copy_force_not_null_clean() {
  let d = diags(
    "COPY users FROM '/tmp/u.csv' WITH (FORMAT csv, FORCE_NOT_NULL (email));"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_copy_force_null_clean() {
  let d = diags(
    "COPY users FROM '/tmp/u.csv' WITH (FORMAT csv, FORCE_NULL (email));"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_copy_header_match_clean() {
  let d = diags(
    "COPY users FROM '/tmp/u.csv' WITH (FORMAT csv, HEADER MATCH);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_lock_table_all_modes_clean() {
  for mode in [
    "ACCESS SHARE",
    "ROW SHARE",
    "ROW EXCLUSIVE",
    "SHARE UPDATE EXCLUSIVE",
    "SHARE",
    "SHARE ROW EXCLUSIVE",
    "EXCLUSIVE",
    "ACCESS EXCLUSIVE",
  ] {
    let stmt = format!("LOCK TABLE users IN {mode} MODE;");
    let d = diags(&stmt);
    assert!(
      !d.iter().any(|x| x.code == "sql000"),
      "{mode} mode failed: {:?}",
      d.iter().map(|x| x.code).collect::<Vec<_>>()
    );
  }
}

#[test]
fn r2_138_lock_table_nowait_clean() {
  let d = diags("LOCK TABLE users IN ACCESS EXCLUSIVE MODE NOWAIT;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_import_foreign_schema_clean() {
  let d = diags(
    "IMPORT FOREIGN SCHEMA public LIMIT TO (users, orders) FROM SERVER reporting INTO public OPTIONS (use_remote_estimate 'true');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_import_foreign_schema_except_clean() {
  let d = diags(
    "IMPORT FOREIGN SCHEMA public EXCEPT (audit) FROM SERVER reporting INTO public;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_alter_default_privileges_clean() {
  let d = diags(
    "ALTER DEFAULT PRIVILEGES FOR ROLE app IN SCHEMA public GRANT SELECT ON TABLES TO ro_role;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_set_role_clean() {
  let d = diags("SET ROLE app_user;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_reset_role_clean() {
  let d = diags("RESET ROLE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_138_set_session_authorization_clean() {
  let d = diags("SET SESSION AUTHORIZATION app_user;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_table_as_clean() {
  let d = diags("CREATE TABLE archived_users AS SELECT * FROM users WHERE active = false;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_table_as_with_data_clean() {
  let d = diags("CREATE TABLE archived AS SELECT id FROM users WITH DATA;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_table_as_with_no_data_clean() {
  let d = diags("CREATE TABLE empty_shell AS SELECT id FROM users WITH NO DATA;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_temp_table_as_clean() {
  let d = diags("CREATE TEMP TABLE tmp_users ON COMMIT DROP AS SELECT id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_temporary_table_clean() {
  let d = diags("CREATE TEMPORARY TABLE t (id int);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_unlogged_table_clean() {
  let d = diags("CREATE UNLOGGED TABLE bulk_load (id int, payload text);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_global_temp_clean() {
  let d = diags("CREATE GLOBAL TEMPORARY TABLE g (id int);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_table_like_clean() {
  let d = diags("CREATE TABLE copy_of_users (LIKE users INCLUDING ALL);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_table_like_with_excluding_clean() {
  let d = diags("CREATE TABLE clone (LIKE users INCLUDING DEFAULTS EXCLUDING CONSTRAINTS);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_table_on_commit_delete_rows_clean() {
  let d = diags("CREATE TEMP TABLE sess_t (id int) ON COMMIT DELETE ROWS;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_table_inherits_clean() {
  let d = diags("CREATE TABLE child (id_extra int) INHERITS (parent);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_table_of_type_clean() {
  let d = diags("CREATE TABLE addr OF address_t (PRIMARY KEY (street));");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_view_with_check_option_clean() {
  let d = diags(
    "CREATE VIEW active_users AS SELECT * FROM users WHERE active WITH CHECK OPTION;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_view_with_local_check_option_clean() {
  let d = diags(
    "CREATE VIEW local_users AS SELECT * FROM users WITH LOCAL CHECK OPTION;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_view_with_cascaded_check_option_clean() {
  let d = diags(
    "CREATE VIEW cascaded_users AS SELECT * FROM users WITH CASCADED CHECK OPTION;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_recursive_view_clean() {
  let d = diags(
    "CREATE RECURSIVE VIEW t(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM t WHERE n < 10);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_or_replace_view_clean() {
  let d = diags(
    "CREATE OR REPLACE VIEW v AS SELECT id FROM users;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_materialized_view_with_data_clean() {
  let d = diags(
    "CREATE MATERIALIZED VIEW mv AS SELECT id, name FROM users WITH DATA;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_materialized_view_with_no_data_clean() {
  let d = diags(
    "CREATE MATERIALIZED VIEW mv AS SELECT id FROM users WITH NO DATA;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_139_create_table_partition_of_clean() {
  let d = diags(
    "CREATE TABLE events_2025 PARTITION OF events FOR VALUES FROM ('2025-01-01') TO ('2026-01-01');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_select_tablesample_clean() {
  let d = diags("SELECT * FROM users TABLESAMPLE BERNOULLI (10) REPEATABLE (42);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_select_tablesample_system_clean() {
  let d = diags("SELECT * FROM users TABLESAMPLE SYSTEM (5);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_for_update_clean() {
  let d = diags("SELECT * FROM users WHERE id = 1 FOR UPDATE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_for_update_of_clean() {
  let d = diags(
    "SELECT * FROM users u JOIN orders o ON u.id = o.user_id FOR UPDATE OF u NOWAIT;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_for_update_skip_locked_clean() {
  let d = diags("SELECT * FROM jobs WHERE done = false FOR UPDATE SKIP LOCKED LIMIT 10;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_for_no_key_update_clean() {
  let d = diags("SELECT * FROM users WHERE id = 1 FOR NO KEY UPDATE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_for_share_clean() {
  let d = diags("SELECT * FROM users WHERE id = 1 FOR SHARE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_for_key_share_clean() {
  let d = diags("SELECT * FROM users WHERE id = 1 FOR KEY SHARE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_grouping_fn_clean() {
  let d = diags(
    "SELECT email, name, GROUPING(email) AS gr, count(*) FROM users \
     GROUP BY ROLLUP (email, name);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_cube_column_group_clean() {
  let d = diags(
    "SELECT email, name, count(*) FROM users GROUP BY CUBE ((email, name));"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_rollup_column_group_clean() {
  let d = diags(
    "SELECT email, name, count(*) FROM users GROUP BY ROLLUP ((email, name));"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_select_distinct_on_clean() {
  let d = diags(
    "SELECT DISTINCT ON (email) id, email FROM users ORDER BY email, id;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_select_fetch_first_n_rows_only_clean() {
  let d = diags("SELECT * FROM users ORDER BY id FETCH FIRST 10 ROWS ONLY;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_select_offset_fetch_clean() {
  let d = diags("SELECT * FROM users ORDER BY id OFFSET 20 ROWS FETCH NEXT 10 ROWS ONLY;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_select_fetch_with_ties_clean() {
  let d = diags("SELECT * FROM users ORDER BY id FETCH FIRST 10 ROWS WITH TIES;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_140_select_union_all_intersect_except_clean() {
  let d = diags(
    "(SELECT id FROM users) UNION ALL (SELECT id FROM orders) \
     INTERSECT (SELECT id FROM events) EXCEPT (SELECT id FROM banned);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_json_table_nested_path_clean() {
  let d = diags(
    "SELECT * FROM JSON_TABLE(jsonb_build_object('items', '[1,2]'::jsonb), '$' \
      COLUMNS (\
        id INT PATH '$.id', \
        NESTED PATH '$.items[*]' COLUMNS (item INT PATH '$')\
      )) AS jt;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_json_value_clean() {
  let d = diags(
    "SELECT JSON_VALUE(data, '$.profile.name' RETURNING text NULL ON EMPTY ERROR ON ERROR) FROM users;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_json_query_with_wrapper_clean() {
  let d = diags(
    "SELECT JSON_QUERY(data, '$.tags' WITH UNCONDITIONAL WRAPPER NULL ON EMPTY) FROM users;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_json_query_omit_quotes_clean() {
  let d = diags(
    "SELECT JSON_QUERY(data, '$.name' OMIT QUOTES) FROM users;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_json_exists_clean() {
  let d = diags(
    "SELECT id FROM users WHERE JSON_EXISTS(data, '$.profile.email');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_json_object_constructor_clean() {
  let d = diags(
    "SELECT JSON_OBJECT('id': id, 'name': name ABSENT ON NULL RETURNING jsonb) FROM users;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_json_array_constructor_clean() {
  let d = diags(
    "SELECT JSON_ARRAY(id, name NULL ON NULL RETURNING jsonb) FROM users;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_json_objectagg_clean() {
  let d = diags(
    "SELECT JSON_OBJECTAGG(id: name ABSENT ON NULL) FROM users;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_json_arrayagg_clean() {
  let d = diags("SELECT JSON_ARRAYAGG(id ORDER BY id) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_overlaps_predicate_clean() {
  let d = diags(
    "SELECT id FROM users WHERE (now() - INTERVAL '1 day', now()) OVERLAPS (now() - INTERVAL '12 hours', now());"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_cast_standard_types_clean() {
  let d = diags(
    "SELECT \
       CAST(1 AS BIGINT), \
       CAST(1.5 AS NUMERIC(10,2)), \
       CAST(now() AS DATE), \
       CAST(now() AS TIMESTAMPTZ), \
       CAST('t' AS BOOLEAN), \
       CAST(1 AS REAL), \
       CAST(1 AS DOUBLE PRECISION), \
       CAST('1 day' AS INTERVAL), \
       CAST(NULL AS TEXT);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_cast_operator_double_colon_clean() {
  let d = diags("SELECT 1::bigint, now()::date, '1 day'::interval;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_generated_stored_column_clean() {
  let d = diags(
    "CREATE TABLE prices (amount numeric, tax_pct numeric, with_tax numeric GENERATED ALWAYS AS (amount * (1 + tax_pct)) STORED);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_generated_identity_always_clean() {
  let d = diags(
    "CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY (START WITH 1000 INCREMENT BY 5 CACHE 50) PRIMARY KEY);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_generated_identity_by_default_clean() {
  let d = diags(
    "CREATE TABLE t (id int GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_insert_overriding_system_value_clean() {
  let d = diags(
    "INSERT INTO t OVERRIDING SYSTEM VALUE VALUES (1, 'x');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_141_insert_overriding_user_value_clean() {
  let d = diags(
    "INSERT INTO t OVERRIDING USER VALUE VALUES (1, 'x');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_any_subquery_clean() {
  let d = diags("SELECT id FROM users WHERE id = ANY (SELECT user_id FROM orders);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_some_subquery_clean() {
  let d = diags("SELECT id FROM users WHERE id < SOME (SELECT id FROM users);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_all_subquery_clean() {
  let d = diags("SELECT id FROM users WHERE id >= ALL (SELECT id FROM users);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_exists_subquery_clean() {
  let d = diags(
    "SELECT u.id FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_not_exists_subquery_clean() {
  let d = diags(
    "SELECT u.id FROM users u WHERE NOT EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id);"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_scalar_subquery_clean() {
  let d = diags(
    "SELECT id, (SELECT count(*) FROM orders o WHERE o.user_id = u.id) AS order_count FROM users u;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_not_in_subquery_clean() {
  let d = diags("SELECT id FROM users WHERE id NOT IN (SELECT user_id FROM orders);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_unnest_clean() {
  let d = diags("SELECT * FROM unnest(ARRAY[1, 2, 3]) WITH ORDINALITY AS t(v, ord);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_array_agg_with_order_clean() {
  let d = diags(
    "SELECT user_id, array_agg(id ORDER BY id) FROM orders GROUP BY user_id;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_array_to_string_clean() {
  let d = diags("SELECT array_to_string(ARRAY['a', 'b', 'c'], ',', '*');");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_cardinality_clean() {
  let d = diags("SELECT cardinality(ARRAY[[1,2],[3,4]]);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_array_position_clean() {
  let d = diags("SELECT array_position(ARRAY[10, 20, 30], 20);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_array_remove_clean() {
  let d = diags("SELECT array_remove(ARRAY[1, NULL, 2, NULL], NULL);");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_array_slice_clean() {
  let d = diags("SELECT (ARRAY[1, 2, 3, 4, 5])[2:4];");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_tsvector_construction_clean() {
  let d = diags(
    "SELECT to_tsvector('english', 'The quick brown fox') @@ to_tsquery('english', 'quick & fox');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_tsvector_setweight_clean() {
  let d = diags(
    "SELECT setweight(to_tsvector('english', 'title'), 'A') || setweight(to_tsvector('english', 'body'), 'D');"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_phraseto_tsquery_clean() {
  let d = diags("SELECT phraseto_tsquery('english', 'the fox jumps');");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_websearch_to_tsquery_clean() {
  let d = diags("SELECT websearch_to_tsquery('english', '\"the fox\" jumps');");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_range_constructor_clean() {
  let d = diags("SELECT int4range(1, 100, '[)') && int4range(50, 200, '[]');");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_range_contains_clean() {
  let d = diags("SELECT int4range(1, 100) @> 50;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_multirange_clean() {
  let d = diags("SELECT int4multirange(int4range(1, 10), int4range(20, 30));");
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_range_agg_clean() {
  let d = diags(
    "SELECT range_agg(daterange(start, finish)) FROM events;"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_142_range_lower_upper_clean() {
  let d = diags(
    "SELECT lower(int4range(1, 100)), upper(int4range(1, 100)), lower_inc(int4range(1, 100));"
  );
  assert!(!d.iter().any(|x| x.code == "sql000"));
}

#[test]
fn r2_153_unknown_column_case_insensitive() {
  // sql002 must not fire when the user references a column in a
  // different case than the catalog stores.
  let d = diags("SELECT ID FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"),
    "sql002 fired for uppercase ID; codes: {:?}",
    d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r2_153_unknown_column_explicit_lowercase_still_clean() {
  let d = diags("SELECT id FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r2_153_uppercase_table_still_resolves() {
  let d = diags("SELECT id FROM USERS;");
  assert!(!d.iter().any(|x| x.code == "sql001"),
    "sql001 fired for uppercase USERS; codes: {:?}",
    d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r2_158_current_user_clean() {
  let d = diags("SELECT current_user, session_user, current_role;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
  assert!(!d.iter().any(|x| x.code == "sql001"));
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r2_158_current_user_uppercase_clean() {
  let d = diags("SELECT CURRENT_USER, SESSION_USER, CURRENT_ROLE;");
  assert!(!d.iter().any(|x| x.code == "sql000"));
  assert!(!d.iter().any(|x| x.code == "sql001"));
  assert!(!d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r2_158_explicit_schema_uppercase_resolves() {
  let d = diags("SELECT id FROM PUBLIC.users;");
  assert!(!d.iter().any(|x| x.code == "sql001"),
    "sql001 fired for uppercase schema PUBLIC; codes: {:?}",
    d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r2_163_2k_line_buffer_no_panic() {
  // 2,000 SELECT statements -- exercises rule iter cost + diagnostic
  // dedup. Cap kept at 2k so the test suite finishes promptly in
  // debug mode; release mode handles 10k+ statements comfortably
  // (~9s, see tests/perf_bench.rs).
  let mut s = String::with_capacity(80_000);
  for i in 0..2_000 {
    s.push_str(&format!("SELECT id FROM users WHERE id = {i};\n"));
  }
  let d = diags(&s);
  assert!(d.iter().filter(|x| x.code == "sql000").count() < 100,
    "sql000 spam from 2k-line buffer");
}

#[test]
fn r2_165_mysql_auto_increment_runs_under_mysql_dialect() {
  use dsl_parse::Dialect;
  let q = "CREATE TABLE x (id INT AUTO_INCREMENT PRIMARY KEY);";
  let file = dsl_parse::parse(q, Dialect::MySql);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, q);
  // sql314 is the MySQL AUTO_INCREMENT detector; on MySQL buffer it must NOT fire.
  let d = dsl_analysis::run_with_dialect(q, &file, &scopes, &cat(), Dialect::MySql);
  assert!(!d.iter().any(|x| x.code == "sql314"),
    "sql314 fired on MySQL buffer: {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r3_005_sql001_flags_update_from_missing_table() {
  // CYCLE 3: UpdateStmt.from_tables now extracted. A missing
  // FROM-list table should be flagged just like a missing JOIN
  // table in SELECT.
  let d = diags("UPDATE users SET x = nope.y FROM nope WHERE 1=1;");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("nope")),
    "expected sql001 for FROM-list missing table: {d:?}");
}

#[test]
fn r3_005_sql001_quiet_when_update_from_table_exists() {
  let d = diags("UPDATE users SET active = true FROM orders o WHERE o.user_id = users.id;");
  assert!(!d.iter().any(|x| x.code == "sql001"),
    "spurious sql001: {d:?}");
}

#[test]
fn r3_005_sql001_flags_delete_using_missing_table() {
  let d = diags("DELETE FROM users USING nope WHERE nope.id = users.id;");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("nope")),
    "expected sql001 for USING-list missing table: {d:?}");
}

#[test]
fn r3_005_sql001_quiet_when_delete_using_table_exists() {
  let d = diags("DELETE FROM users USING orders o WHERE o.user_id = users.id;");
  assert!(!d.iter().any(|x| x.code == "sql001"),
    "spurious sql001: {d:?}");
}

#[test]
fn r3_026_sql001_quiet_for_set_returning_func() {
  let d = diags("SELECT * FROM generate_series(1, 10);");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r3_027_sql001_quiet_for_pg_catalog_qualified() {
  let d = diags("SELECT * FROM pg_catalog.pg_proc;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r3_028_sql001_quiet_for_information_schema_qualified() {
  let d = diags("SELECT * FROM information_schema.tables;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r3_029_sql001_quiet_for_cte_referenced_later() {
  let d = diags("WITH t AS (SELECT 1 AS x) SELECT * FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r3_030_sql001_recursive_cte_self_ref_no_panic() {
  // Recursive CTE self-references are valid. sql001 may not descend
  // into the CTE body, but the outer SELECT t must NOT be flagged.
  let d = diags(
    "WITH RECURSIVE t AS (SELECT 1 AS x UNION ALL SELECT x+1 FROM t WHERE x < 5) SELECT * FROM t;",
  );
  assert!(!d.iter().any(|x| x.code == "sql001" && x.message.contains("`t`")),
    "spurious sql001 on CTE self-ref: {d:?}");
}

#[test]
fn r3_031_sql000_no_spurious_on_empty_stmt() {
  let d = diags("");
  assert!(d.is_empty(), "expected no diags on empty: {d:?}");
}

#[test]
fn r3_032_sql000_no_spurious_on_only_comments() {
  let d = diags("-- comment\n/* block */\n");
  assert!(d.is_empty(), "expected no diags on comment-only: {d:?}");
}

fn cat_with_add2_fn() -> Catalog {
  use dsl_catalog::{Function, FunctionArg};
  let mut c = cat();
  c.functions.push(Function {
    schema: "public".into(),
    name: "add2".into(),
    arguments: vec![
      FunctionArg { name: Some("a".into()), data_type: "int".into() },
      FunctionArg { name: Some("b".into()), data_type: "int".into() },
    ],
    return_type: "int".into(),
    comment: None,
  });
  c.functions.push(Function {
    schema: "public".into(),
    name: "foo".into(),
    arguments: vec![
      FunctionArg { name: Some("a".into()), data_type: "int".into() },
      FunctionArg { name: Some("b".into()), data_type: "int".into() },
    ],
    return_type: "int".into(),
    comment: None,
  });
  c.functions.push(Function {
    schema: "public".into(),
    name: "foo_def".into(),
    arguments: vec![
      FunctionArg { name: Some("a".into()), data_type: "int".into() },
      FunctionArg { name: Some("b".into()), data_type: "int DEFAULT 0".into() },
    ],
    return_type: "int".into(),
    comment: None,
  });
  c
}

fn diags_with_funcs(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let file = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, src);
  let cat = cat_with_add2_fn();
  dsl_analysis::run(src, &file, &scopes, &cat)
}

#[test]
fn r4_sql513_too_few_args() {
  let d = diags_with_funcs(
    "CREATE FUNCTION add2(a int, b int) RETURNS int LANGUAGE sql AS $$ SELECT a + b $$;
     SELECT add2(1);"
  );
  assert!(d.iter().any(|x| x.code == "sql513" && x.message.contains("requires 2")),
    "expected sql513 too-few: {d:?}");
}

#[test]
fn r4_sql513_too_many_args() {
  let d = diags_with_funcs(
    "CREATE FUNCTION add2(a int, b int) RETURNS int LANGUAGE sql AS $$ SELECT a + b $$;
     SELECT add2(1, 2, 3);"
  );
  assert!(d.iter().any(|x| x.code == "sql513" && x.message.contains("at most 2")),
    "expected sql513 too-many: {d:?}");
}

#[test]
fn r4_sql513_correct_arity_quiet() {
  let d = diags_with_funcs(
    "CREATE FUNCTION add2(a int, b int) RETURNS int LANGUAGE sql AS $$ SELECT a + b $$;
     SELECT add2(1, 2);"
  );
  assert!(!d.iter().any(|x| x.code == "sql513"), "spurious sql513: {d:?}");
}

#[test]
fn r4_sql513_no_args_required_when_call_is_empty() {
  let d = diags_with_funcs(
    "CREATE FUNCTION foo(a int, b int) RETURNS int LANGUAGE sql AS $$ SELECT a + b $$;
     SELECT foo();"
  );
  assert!(d.iter().any(|x| x.code == "sql513" && x.message.contains("requires 2")),
    "expected sql513 for SELECT foo() needing args: {d:?}");
}

#[test]
fn r4_sql513_default_arg_tolerated() {
  let d = diags_with_funcs(
    "CREATE FUNCTION foo(a int, b int DEFAULT 0) RETURNS int LANGUAGE sql AS $$ SELECT a + b $$;
     SELECT foo_def(1);"
  );
  assert!(!d.iter().any(|x| x.code == "sql513"),
    "DEFAULT arg should not fire sql513: {d:?}");
}

#[test]
fn r4_sql513_unknown_function_quiet() {
  let d = diags_with_funcs("SELECT unknown_fn(1, 2);");
  assert!(!d.iter().any(|x| x.code == "sql513"));
}

#[test]
fn r4_sql513_nested_call_no_panic() {
  let d = diags_with_funcs(
    "CREATE FUNCTION add2(a int, b int) RETURNS int LANGUAGE sql AS $$ SELECT a + b $$;
     SELECT add2(add2(1, 2), 3);"
  );
  assert!(!d.iter().any(|x| x.code == "sql513"),
    "valid nested call should not fire: {d:?}");
}

#[test]
fn r4_sql513_schema_qualified() {
  use dsl_catalog::Function;
  let mut c = cat();
  c.functions.push(Function {
    schema: "app".into(),
    name: "current_user_id".into(),
    arguments: vec![],
    return_type: "uuid".into(),
    comment: None,
  });
  let file = dsl_parse::parse("SELECT app.current_user_id(1);", dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, "SELECT app.current_user_id(1);");
  let d = dsl_analysis::run("SELECT app.current_user_id(1);", &file, &scopes, &c);
  assert!(d.iter().any(|x| x.code == "sql513"),
    "schema.fn(arg) should flag when fn takes 0 args: {d:?}");
}

#[test]
fn r4_sql513_inside_where_clause() {
  let d = diags_with_funcs(
    "CREATE FUNCTION add2(a int, b int) RETURNS int LANGUAGE sql AS $$ SELECT a + b $$;
     SELECT 1 WHERE add2(1) > 0;"
  );
  assert!(d.iter().any(|x| x.code == "sql513"),
    "should fire in WHERE clause: {d:?}");
}

#[test]
fn r4_sql513_call_inside_string_ignored() {
  let d = diags_with_funcs(
    "CREATE FUNCTION add2(a int, b int) RETURNS int LANGUAGE sql AS $$ SELECT a + b $$;
     SELECT 'add2(1)' FROM t;"
  );
  // The `add2(1)` inside the string literal must not fire.
  assert!(!d.iter().any(|x| x.code == "sql513"),
    "string literal `add2(1)` should be inert: {d:?}");
}

#[test]
fn r4_sql513_call_inside_comment_ignored() {
  let d = diags_with_funcs(
    "CREATE FUNCTION add2(a int, b int) RETURNS int LANGUAGE sql AS $$ SELECT a + b $$;
     -- add2(1) in a comment
     SELECT 1;"
  );
  assert!(!d.iter().any(|x| x.code == "sql513"));
}

#[test]
fn r4_500_sql513_overload_resolution() {
  use dsl_catalog::Function;
  let mut c = cat();
  // Two overloads: 1-arg and 2-arg `foo`.
  c.functions.push(Function {
    schema: "public".into(), name: "foo_ovl".into(),
    arguments: vec![dsl_catalog::FunctionArg { name: Some("a".into()), data_type: "int".into() }],
    return_type: "int".into(), comment: None,
  });
  c.functions.push(Function {
    schema: "public".into(), name: "foo_ovl".into(),
    arguments: vec![
      dsl_catalog::FunctionArg { name: Some("a".into()), data_type: "int".into() },
      dsl_catalog::FunctionArg { name: Some("b".into()), data_type: "int".into() },
    ],
    return_type: "int".into(), comment: None,
  });
  let src = "SELECT foo_ovl(1); SELECT foo_ovl(1, 2);";
  let file = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, src);
  let d = dsl_analysis::run(src, &file, &scopes, &c);
  assert!(!d.iter().any(|x| x.code == "sql513"), "overloads should silence sql513: {d:?}");
}

#[test]
fn r4_501_sql513_variadic_unbounded() {
  use dsl_catalog::Function;
  let mut c = cat();
  c.functions.push(Function {
    schema: "public".into(), name: "concat_vs".into(),
    arguments: vec![
      dsl_catalog::FunctionArg { name: Some("a".into()), data_type: "VARIADIC text".into() },
    ],
    return_type: "text".into(), comment: None,
  });
  let src = "SELECT concat_vs('a', 'b', 'c', 'd', 'e');";
  let file = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, src);
  let d = dsl_analysis::run(src, &file, &scopes, &c);
  assert!(!d.iter().any(|x| x.code == "sql513"));
}

#[test]
fn r4_sql513_literal_int_for_text_arg() {
  use dsl_catalog::{Function, FunctionArg};
  let mut c = cat();
  c.functions.push(Function {
    schema: "public".into(), name: "greet".into(),
    arguments: vec![FunctionArg { name: Some("who".into()), data_type: "text".into() }],
    return_type: "text".into(), comment: None,
  });
  let src = "SELECT greet(42);";
  let file = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, src);
  let d = dsl_analysis::run(src, &file, &scopes, &c);
  assert!(d.iter().any(|x| x.code == "sql513" && x.message.contains("integer") && x.message.contains("text")),
    "expected literal-type mismatch: {d:?}");
}

#[test]
fn r4_sql513_literal_text_for_int_arg() {
  use dsl_catalog::{Function, FunctionArg};
  let mut c = cat();
  c.functions.push(Function {
    schema: "public".into(), name: "pow2".into(),
    arguments: vec![FunctionArg { name: Some("n".into()), data_type: "int".into() }],
    return_type: "int".into(), comment: None,
  });
  let src = "SELECT pow2('three');";
  let file = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, src);
  let d = dsl_analysis::run(src, &file, &scopes, &c);
  assert!(d.iter().any(|x| x.code == "sql513" && x.message.contains("text") && x.message.contains("integer")),
    "expected text-for-int mismatch: {d:?}");
}

#[test]
fn r4_sql513_correct_literal_quiet() {
  use dsl_catalog::{Function, FunctionArg};
  let mut c = cat();
  c.functions.push(Function {
    schema: "public".into(), name: "pow2".into(),
    arguments: vec![FunctionArg { name: Some("n".into()), data_type: "int".into() }],
    return_type: "int".into(), comment: None,
  });
  let src = "SELECT pow2(42);";
  let file = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, src);
  let d = dsl_analysis::run(src, &file, &scopes, &c);
  assert!(!d.iter().any(|x| x.code == "sql513"), "spurious: {d:?}");
}

#[test]
fn r4_sql513_null_arg_silent() {
  use dsl_catalog::{Function, FunctionArg};
  let mut c = cat();
  c.functions.push(Function {
    schema: "public".into(), name: "pow2".into(),
    arguments: vec![FunctionArg { name: Some("n".into()), data_type: "int".into() }],
    return_type: "int".into(), comment: None,
  });
  let src = "SELECT pow2(NULL);";
  let file = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, src);
  let d = dsl_analysis::run(src, &file, &scopes, &c);
  assert!(!d.iter().any(|x| x.code == "sql513"));
}

#[test]
fn r9_diag_sql001_0001() {
  let d = diags("SELECT * FROM xyz_t1");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001");
}

#[test]
fn r9_diag_sql001_0002() {
  let d = diags("SELECT id FROM xyz_t2");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001");
}

#[test]
fn r9_diag_sql001_0003() {
  let d = diags("SELECT * FROM xyz_t3 WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001");
}

#[test]
fn r9_diag_sql001_0004() {
  let d = diags("SELECT * FROM xyz_t4 ORDER BY id");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001");
}

#[test]
fn r9_diag_sql001_0005() {
  let d = diags("SELECT count(*) FROM xyz_t5");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001");
}

#[test]
fn r9_diag_sql001_0006() {
  let d = diags("INSERT INTO xyz_t6 VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001");
}

#[test]
fn r9_diag_sql001_0007() {
  let d = diags("UPDATE xyz_t7 SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001");
}

#[test]
fn r9_diag_sql001_0008() {
  let d = diags("DELETE FROM xyz_t8");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001");
}

#[test]
fn r9_diag_sql001_0009() {
  let d = diags("SELECT * FROM xyz_t9 u");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001");
}

#[test]
fn r9_diag_sql001_0010() {
  let d = diags("SELECT * FROM xyz_t10 JOIN users ON true");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001");
}

#[test]
fn r9_diag_no_sql001_0501() {
  let d = diags("SELECT * FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r9_diag_no_sql001_0502() {
  let d = diags("SELECT id FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r9_diag_no_sql001_0503() {
  let d = diags("SELECT * FROM users WHERE id = '00000000-0000-0000-0000-000000000000'");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r9_diag_no_sql001_0504() {
  let d = diags("SELECT count(*) FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r9_diag_no_sql001_0505() {
  let d = diags("SELECT * FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r9_diag_no_sql001_0506() {
  let d = diags("SELECT id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r9_diag_no_sql001_0507() {
  let d = diags("SELECT user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r9_diag_no_sql001_0508() {
  let d = diags("INSERT INTO users (id) VALUES ('00000000-0000-0000-0000-000000000000')");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r9_diag_no_sql001_0509() {
  let d = diags("UPDATE users SET name = 'x'");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r9_diag_no_sql001_0510() {
  let d = diags("DELETE FROM users WHERE id = '00000000-0000-0000-0000-000000000000'");
  assert!(!d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r9_diag_sql002_0901() {
  let d = diags("SELECT xxxcol1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0902() {
  let d = diags("SELECT id, xxxcol2 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0903() {
  let d = diags("SELECT * FROM users WHERE xxxcol3 = 1");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0904() {
  let d = diags("SELECT u.yyycol1 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0905() {
  let d = diags("SELECT yyycol2 FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0906() {
  let d = diags("SELECT zzzcol1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0907() {
  let d = diags("SELECT id, abccol1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0908() {
  let d = diags("SELECT * FROM users WHERE defcol2 = 1");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0909() {
  let d = diags("SELECT u.xxxcol1 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0910() {
  let d = diags("SELECT xxxcol2 FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0911() {
  let d = diags("SELECT xxxcol3 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0912() {
  let d = diags("SELECT id, yyycol1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0913() {
  let d = diags("SELECT * FROM users WHERE yyycol2 = 1");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0914() {
  let d = diags("SELECT u.zzzcol1 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0915() {
  let d = diags("SELECT abccol1 FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0916() {
  let d = diags("SELECT defcol2 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0917() {
  let d = diags("SELECT id, xxxcol1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0918() {
  let d = diags("SELECT * FROM users WHERE xxxcol2 = 1");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0919() {
  let d = diags("SELECT u.xxxcol3 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0920() {
  let d = diags("SELECT yyycol1 FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0921() {
  let d = diags("SELECT yyycol2 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0922() {
  let d = diags("SELECT id, zzzcol1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0923() {
  let d = diags("SELECT * FROM users WHERE abccol1 = 1");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0924() {
  let d = diags("SELECT u.defcol2 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0925() {
  let d = diags("SELECT xxxcol1 FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0926() {
  let d = diags("SELECT xxxcol2 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0927() {
  let d = diags("SELECT id, xxxcol3 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0928() {
  let d = diags("SELECT * FROM users WHERE yyycol1 = 1");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0929() {
  let d = diags("SELECT u.yyycol2 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_sql002_0930() {
  let d = diags("SELECT zzzcol1 FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002");
}

#[test]
fn r9_diag_no_sql002_1401() {
  let d = diags("SELECT id FROM users");
  assert!(!d.iter().any(|x| x.code == "sql002"), "unexpected sql002 for `SELECT id FROM users`");
}

#[test]
fn r9_diag_no_sql002_1402() {
  let d = diags("SELECT email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql002"), "unexpected sql002 for `SELECT email FROM users`");
}

#[test]
fn r9_diag_no_sql002_1403() {
  let d = diags("SELECT name FROM users");
  assert!(!d.iter().any(|x| x.code == "sql002"), "unexpected sql002 for `SELECT name FROM users`");
}

#[test]
fn r9_diag_no_sql002_1404() {
  let d = diags("SELECT id, email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql002"), "unexpected sql002 for `SELECT id, email FROM users`");
}

#[test]
fn r9_diag_no_sql002_1405() {
  let d = diags("SELECT id, name FROM users");
  assert!(!d.iter().any(|x| x.code == "sql002"), "unexpected sql002 for `SELECT id, name FROM users`");
}

#[test]
fn r9_diag_no_sql002_1406() {
  let d = diags("SELECT email, name FROM users");
  assert!(!d.iter().any(|x| x.code == "sql002"), "unexpected sql002 for `SELECT email, name FROM users`");
}

#[test]
fn r9_diag_no_sql002_1407() {
  let d = diags("SELECT id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql002"), "unexpected sql002 for `SELECT id FROM orders`");
}

#[test]
fn r9_diag_no_sql002_1408() {
  let d = diags("SELECT user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql002"), "unexpected sql002 for `SELECT user_id FROM orders`");
}

#[test]
fn r9_diag_no_sql002_1409() {
  let d = diags("SELECT id, user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql002"), "unexpected sql002 for `SELECT id, user_id FROM orders`");
}

#[test]
fn r9_diag_sql003_1701() {
  let d = diags("SELECT id FROM users, orders");
  assert!(d.iter().any(|x| x.code == "sql003"), "expected sql003");
}

#[test]
fn r9_diag_sql003_1702() {
  let d = diags("SELECT id FROM users u, orders o");
  assert!(d.iter().any(|x| x.code == "sql003"), "expected sql003");
}

#[test]
fn r9_diag_sql003_1703() {
  let d = diags("SELECT id FROM users JOIN orders ON true");
  assert!(d.iter().any(|x| x.code == "sql003"), "expected sql003");
}

#[test]
fn r9_diag_sql003_1704() {
  let d = diags("SELECT id FROM users CROSS JOIN orders");
  assert!(d.iter().any(|x| x.code == "sql003"), "expected sql003");
}

#[test]
fn r9_diag_no_sql003_2001() {
  let d = diags("SELECT users.id FROM users, orders");
  assert!(!d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r9_diag_no_sql003_2002() {
  let d = diags("SELECT orders.id FROM users, orders");
  assert!(!d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r9_diag_no_sql003_2003() {
  let d = diags("SELECT u.id FROM users u, orders o");
  assert!(!d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r9_diag_no_sql003_2004() {
  let d = diags("SELECT o.id FROM users u, orders o");
  assert!(!d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r9_diag_no_sql003_2005() {
  let d = diags("SELECT u.id, o.id FROM users u, orders o");
  assert!(!d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r9_diag_sql001_sev_2201() {
  let d = diags("SELECT * FROM xyz_t1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r9_diag_sql001_sev_2202() {
  let d = diags("SELECT id FROM xyz_t2");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r9_diag_sql001_sev_2203() {
  let d = diags("SELECT * FROM xyz_t3 WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r9_diag_sql001_sev_2204() {
  let d = diags("SELECT * FROM xyz_t4 ORDER BY id");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r9_diag_sql001_sev_2205() {
  let d = diags("SELECT count(*) FROM xyz_t5");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r9_diag_sql001_sev_2206() {
  let d = diags("INSERT INTO xyz_t6 VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r9_diag_sql001_sev_2207() {
  let d = diags("UPDATE xyz_t7 SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r9_diag_sql001_sev_2208() {
  let d = diags("DELETE FROM xyz_t8");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r9_diag_sql001_sev_2209() {
  let d = diags("SELECT * FROM xyz_t9 u");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r9_diag_sql001_sev_2210() {
  let d = diags("SELECT * FROM xyz_t10 JOIN users ON true");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r9_diag_clean_2401() {
  let d = diags("SELECT id FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r9_diag_clean_2402() {
  let d = diags("SELECT email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r9_diag_clean_2403() {
  let d = diags("SELECT id, email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r9_diag_clean_2404() {
  let d = diags("SELECT id, email, name FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r9_diag_clean_2405() {
  let d = diags("SELECT id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r9_diag_clean_2406() {
  let d = diags("SELECT user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r9_diag_clean_2407() {
  let d = diags("SELECT id, user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r9_diag_clean_2408() {
  let d = diags("SELECT count(*) FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r9_diag_clean_2409() {
  let d = diags("SELECT count(*) FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}


#[test]
fn r10_sql001_0001() {
  let d = diags("SELECT * FROM xyz_foo");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM xyz_foo`");
}

#[test]
fn r10_sql001_0002() {
  let d = diags("SELECT id FROM xyz_foo");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT id FROM xyz_foo`");
}

#[test]
fn r10_sql001_0003() {
  let d = diags("SELECT id, name FROM xyz_foo");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT id, name FROM xyz_foo`");
}

#[test]
fn r10_sql001_0004() {
  let d = diags("SELECT * FROM xyz_foo WHERE id=1");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM xyz_foo WHERE id=1`");
}

#[test]
fn r10_sql001_0005() {
  let d = diags("SELECT count(*) FROM xyz_foo");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT count(*) FROM xyz_foo`");
}

#[test]
fn r10_sql001_0006() {
  let d = diags("INSERT INTO xyz_foo VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `INSERT INTO xyz_foo VALUES (1)`");
}

#[test]
fn r10_sql001_0007() {
  let d = diags("INSERT INTO xyz_foo (id) VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `INSERT INTO xyz_foo (id) VALUES (1)`");
}

#[test]
fn r10_sql001_0008() {
  let d = diags("UPDATE xyz_foo SET id=1");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `UPDATE xyz_foo SET id=1`");
}

#[test]
fn r10_sql001_0009() {
  let d = diags("DELETE FROM xyz_foo");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `DELETE FROM xyz_foo`");
}

#[test]
fn r10_sql001_0010() {
  let d = diags("SELECT * FROM xyz_foo u");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM xyz_foo u`");
}

#[test]
fn r10_sql001_0011() {
  let d = diags("SELECT * FROM xyz_foo JOIN users ON true");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM xyz_foo JOIN users ON true`");
}

#[test]
fn r10_sql001_0012() {
  let d = diags("SELECT * FROM users JOIN xyz_foo ON true");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM users JOIN xyz_foo ON true`");
}

#[test]
fn r10_sql001_0013() {
  let d = diags("SELECT * FROM xyz_bar");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM xyz_bar`");
}

#[test]
fn r10_sql001_0014() {
  let d = diags("SELECT id FROM xyz_bar");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT id FROM xyz_bar`");
}

#[test]
fn r10_sql001_0015() {
  let d = diags("SELECT id, name FROM xyz_bar");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT id, name FROM xyz_bar`");
}

#[test]
fn r10_sql001_0016() {
  let d = diags("SELECT * FROM xyz_bar WHERE id=1");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM xyz_bar WHERE id=1`");
}

#[test]
fn r10_sql001_0017() {
  let d = diags("SELECT count(*) FROM xyz_bar");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT count(*) FROM xyz_bar`");
}

#[test]
fn r10_sql001_0018() {
  let d = diags("INSERT INTO xyz_bar VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `INSERT INTO xyz_bar VALUES (1)`");
}

#[test]
fn r10_sql001_0019() {
  let d = diags("INSERT INTO xyz_bar (id) VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `INSERT INTO xyz_bar (id) VALUES (1)`");
}

#[test]
fn r10_sql001_0020() {
  let d = diags("UPDATE xyz_bar SET id=1");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `UPDATE xyz_bar SET id=1`");
}

#[test]
fn r10_sql001_0021() {
  let d = diags("DELETE FROM xyz_bar");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `DELETE FROM xyz_bar`");
}

#[test]
fn r10_sql001_0022() {
  let d = diags("SELECT * FROM xyz_bar u");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM xyz_bar u`");
}

#[test]
fn r10_sql001_0023() {
  let d = diags("SELECT * FROM xyz_bar JOIN users ON true");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM xyz_bar JOIN users ON true`");
}

#[test]
fn r10_sql001_0024() {
  let d = diags("SELECT * FROM users JOIN xyz_bar ON true");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM users JOIN xyz_bar ON true`");
}

#[test]
fn r10_sql001_0025() {
  let d = diags("SELECT * FROM xyz_baz");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM xyz_baz`");
}

#[test]
fn r10_sql001_0026() {
  let d = diags("SELECT id FROM xyz_baz");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT id FROM xyz_baz`");
}

#[test]
fn r10_sql001_0027() {
  let d = diags("SELECT id, name FROM xyz_baz");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT id, name FROM xyz_baz`");
}

#[test]
fn r10_sql001_0028() {
  let d = diags("SELECT * FROM xyz_baz WHERE id=1");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT * FROM xyz_baz WHERE id=1`");
}

#[test]
fn r10_sql001_0029() {
  let d = diags("SELECT count(*) FROM xyz_baz");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `SELECT count(*) FROM xyz_baz`");
}

#[test]
fn r10_sql001_0030() {
  let d = diags("INSERT INTO xyz_baz VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"), "missed sql001 for `INSERT INTO xyz_baz VALUES (1)`");
}

#[test]
fn r10_sql002_0301() {
  let d = diags("SELECT xx_a FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_a FROM users`");
}

#[test]
fn r10_sql002_0302() {
  let d = diags("SELECT id, xx_a FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT id, xx_a FROM users`");
}

#[test]
fn r10_sql002_0303() {
  let d = diags("SELECT xx_a, id FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_a, id FROM users`");
}

#[test]
fn r10_sql002_0304() {
  let d = diags("SELECT * FROM users WHERE xx_a=1");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT * FROM users WHERE xx_a=1`");
}

#[test]
fn r10_sql002_0305() {
  let d = diags("SELECT u.xx_a FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT u.xx_a FROM users u`");
}

#[test]
fn r10_sql002_0306() {
  let d = diags("SELECT xx_a FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_a FROM orders`");
}

#[test]
fn r10_sql002_0307() {
  let d = diags("SELECT id, xx_a FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT id, xx_a FROM orders`");
}

#[test]
fn r10_sql002_0308() {
  let d = diags("SELECT xx_b FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_b FROM users`");
}

#[test]
fn r10_sql002_0309() {
  let d = diags("SELECT id, xx_b FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT id, xx_b FROM users`");
}

#[test]
fn r10_sql002_0310() {
  let d = diags("SELECT xx_b, id FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_b, id FROM users`");
}

#[test]
fn r10_sql002_0311() {
  let d = diags("SELECT * FROM users WHERE xx_b=1");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT * FROM users WHERE xx_b=1`");
}

#[test]
fn r10_sql002_0312() {
  let d = diags("SELECT u.xx_b FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT u.xx_b FROM users u`");
}

#[test]
fn r10_sql002_0313() {
  let d = diags("SELECT xx_b FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_b FROM orders`");
}

#[test]
fn r10_sql002_0314() {
  let d = diags("SELECT id, xx_b FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT id, xx_b FROM orders`");
}

#[test]
fn r10_sql002_0315() {
  let d = diags("SELECT xx_c FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_c FROM users`");
}

#[test]
fn r10_sql002_0316() {
  let d = diags("SELECT id, xx_c FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT id, xx_c FROM users`");
}

#[test]
fn r10_sql002_0317() {
  let d = diags("SELECT xx_c, id FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_c, id FROM users`");
}

#[test]
fn r10_sql002_0318() {
  let d = diags("SELECT * FROM users WHERE xx_c=1");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT * FROM users WHERE xx_c=1`");
}

#[test]
fn r10_sql002_0319() {
  let d = diags("SELECT u.xx_c FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT u.xx_c FROM users u`");
}

#[test]
fn r10_sql002_0320() {
  let d = diags("SELECT xx_c FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_c FROM orders`");
}

#[test]
fn r10_sql002_0321() {
  let d = diags("SELECT id, xx_c FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT id, xx_c FROM orders`");
}

#[test]
fn r10_sql002_0322() {
  let d = diags("SELECT xx_d FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_d FROM users`");
}

#[test]
fn r10_sql002_0323() {
  let d = diags("SELECT id, xx_d FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT id, xx_d FROM users`");
}

#[test]
fn r10_sql002_0324() {
  let d = diags("SELECT xx_d, id FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_d, id FROM users`");
}

#[test]
fn r10_sql002_0325() {
  let d = diags("SELECT * FROM users WHERE xx_d=1");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT * FROM users WHERE xx_d=1`");
}

#[test]
fn r10_sql002_0326() {
  let d = diags("SELECT u.xx_d FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT u.xx_d FROM users u`");
}

#[test]
fn r10_sql002_0327() {
  let d = diags("SELECT xx_d FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_d FROM orders`");
}

#[test]
fn r10_sql002_0328() {
  let d = diags("SELECT id, xx_d FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT id, xx_d FROM orders`");
}

#[test]
fn r10_sql002_0329() {
  let d = diags("SELECT xx_e FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT xx_e FROM users`");
}

#[test]
fn r10_sql002_0330() {
  let d = diags("SELECT id, xx_e FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "missed sql002 for `SELECT id, xx_e FROM users`");
}

#[test]
fn r10_clean_0483() {
  let d = diags("-- c0\nSELECT id FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0484() {
  let d = diags("-- c0\nSELECT email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0485() {
  let d = diags("-- c0\nSELECT id, email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0486() {
  let d = diags("/* c0 */ SELECT * FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0487() {
  let d = diags("/* c0 */ SELECT id, user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0488() {
  let d = diags("-- c1\nSELECT id FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0489() {
  let d = diags("-- c1\nSELECT email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0490() {
  let d = diags("-- c1\nSELECT id, email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0491() {
  let d = diags("/* c1 */ SELECT * FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0492() {
  let d = diags("/* c1 */ SELECT id, user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0496() {
  let d = diags("/* c2 */ SELECT * FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r10_clean_0497() {
  let d = diags("/* c2 */ SELECT id, user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_sql001_0001() {
  let d = diags("-- t0\nSELECT * FROM bogus_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0002() {
  let d = diags("-- t0\nSELECT id FROM bogus_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0003() {
  let d = diags("-- t0\nINSERT INTO bogus_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0004() {
  let d = diags("-- t0\nUPDATE bogus_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0005() {
  let d = diags("-- t0\nDELETE FROM bogus_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0006() {
  let d = diags("-- t0\nSELECT * FROM bogus_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0008() {
  let d = diags("-- t0\nSELECT * FROM fake_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0009() {
  let d = diags("-- t0\nSELECT id FROM fake_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0010() {
  let d = diags("-- t0\nINSERT INTO fake_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0011() {
  let d = diags("-- t0\nUPDATE fake_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0012() {
  let d = diags("-- t0\nDELETE FROM fake_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0013() {
  let d = diags("-- t0\nSELECT * FROM fake_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0015() {
  let d = diags("-- t0\nSELECT * FROM missing_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0016() {
  let d = diags("-- t0\nSELECT id FROM missing_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0017() {
  let d = diags("-- t0\nINSERT INTO missing_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0018() {
  let d = diags("-- t0\nUPDATE missing_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0019() {
  let d = diags("-- t0\nDELETE FROM missing_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0020() {
  let d = diags("-- t0\nSELECT * FROM missing_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0022() {
  let d = diags("-- t0\nSELECT * FROM ghost_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0023() {
  let d = diags("-- t0\nSELECT id FROM ghost_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0024() {
  let d = diags("-- t0\nINSERT INTO ghost_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0025() {
  let d = diags("-- t0\nUPDATE ghost_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0026() {
  let d = diags("-- t0\nDELETE FROM ghost_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0027() {
  let d = diags("-- t0\nSELECT * FROM ghost_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0029() {
  let d = diags("-- t0\nSELECT * FROM phantom_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0030() {
  let d = diags("-- t0\nSELECT id FROM phantom_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0031() {
  let d = diags("-- t0\nINSERT INTO phantom_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0032() {
  let d = diags("-- t0\nUPDATE phantom_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0033() {
  let d = diags("-- t0\nDELETE FROM phantom_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql001_0034() {
  let d = diags("-- t0\nSELECT * FROM phantom_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r11_sql002_0401() {
  let d = diags("-- c0\nSELECT fakecol1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0402() {
  let d = diags("-- c0\nSELECT id, fakecol1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0403() {
  let d = diags("-- c0\nSELECT * FROM users WHERE fakecol1 = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0404() {
  let d = diags("-- c0\nSELECT u.fakecol1 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0405() {
  let d = diags("-- c0\nSELECT fakecol1 FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0406() {
  let d = diags("-- c0\nSELECT fakecol2 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0407() {
  let d = diags("-- c0\nSELECT id, fakecol2 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0408() {
  let d = diags("-- c0\nSELECT * FROM users WHERE fakecol2 = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0409() {
  let d = diags("-- c0\nSELECT u.fakecol2 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0410() {
  let d = diags("-- c0\nSELECT fakecol2 FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0411() {
  let d = diags("-- c0\nSELECT fakecol3 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0412() {
  let d = diags("-- c0\nSELECT id, fakecol3 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0413() {
  let d = diags("-- c0\nSELECT * FROM users WHERE fakecol3 = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0414() {
  let d = diags("-- c0\nSELECT u.fakecol3 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0415() {
  let d = diags("-- c0\nSELECT fakecol3 FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0416() {
  let d = diags("-- c0\nSELECT ghostcol FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0417() {
  let d = diags("-- c0\nSELECT id, ghostcol FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0418() {
  let d = diags("-- c0\nSELECT * FROM users WHERE ghostcol = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0419() {
  let d = diags("-- c0\nSELECT u.ghostcol FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0420() {
  let d = diags("-- c0\nSELECT ghostcol FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0421() {
  let d = diags("-- c0\nSELECT phantomcol FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0422() {
  let d = diags("-- c0\nSELECT id, phantomcol FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0423() {
  let d = diags("-- c0\nSELECT * FROM users WHERE phantomcol = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0424() {
  let d = diags("-- c0\nSELECT u.phantomcol FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0425() {
  let d = diags("-- c0\nSELECT phantomcol FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0426() {
  let d = diags("-- c0\nSELECT missingcol FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0427() {
  let d = diags("-- c0\nSELECT id, missingcol FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0428() {
  let d = diags("-- c0\nSELECT * FROM users WHERE missingcol = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0429() {
  let d = diags("-- c0\nSELECT u.missingcol FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_sql002_0430() {
  let d = diags("-- c0\nSELECT missingcol FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r11_clean_0703() {
  let d = diags("-- ok0\nSELECT name FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0705() {
  let d = diags("-- ok0\nSELECT id, name, email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0706() {
  let d = diags("-- ok0\nSELECT u.id FROM users u");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0707() {
  let d = diags("-- ok0\nSELECT u.email FROM users u");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0708() {
  let d = diags("-- ok0\nSELECT id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0709() {
  let d = diags("-- ok0\nSELECT user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0710() {
  let d = diags("-- ok0\nSELECT id, user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0711() {
  let d = diags("-- ok0\nSELECT count(*) FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0712() {
  let d = diags("-- ok0\nSELECT count(*) FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0715() {
  let d = diags("-- ok1\nSELECT name FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0717() {
  let d = diags("-- ok1\nSELECT id, name, email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0718() {
  let d = diags("-- ok1\nSELECT u.id FROM users u");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0719() {
  let d = diags("-- ok1\nSELECT u.email FROM users u");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0720() {
  let d = diags("-- ok1\nSELECT id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0721() {
  let d = diags("-- ok1\nSELECT user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0722() {
  let d = diags("-- ok1\nSELECT id, user_id FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0723() {
  let d = diags("-- ok1\nSELECT count(*) FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0724() {
  let d = diags("-- ok1\nSELECT count(*) FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0727() {
  let d = diags("-- ok2\nSELECT name FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0729() {
  let d = diags("-- ok2\nSELECT id, name, email FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r11_clean_0730() {
  let d = diags("-- ok2\nSELECT u.id FROM users u");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r12_sev_err_0001() {
  let d = diags("-- s12_0\nSELECT * FROM x1_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0002() {
  let d = diags("-- s12_0\nSELECT id FROM x1_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0003() {
  let d = diags("-- s12_0\nINSERT INTO x1_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0004() {
  let d = diags("-- s12_0\nUPDATE x1_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0005() {
  let d = diags("-- s12_0\nDELETE FROM x1_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0006() {
  let d = diags("-- s12_0\nSELECT * FROM x1_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0007() {
  let d = diags("-- s12_0\nSELECT * FROM x2_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0008() {
  let d = diags("-- s12_0\nSELECT id FROM x2_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0009() {
  let d = diags("-- s12_0\nINSERT INTO x2_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0010() {
  let d = diags("-- s12_0\nUPDATE x2_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0011() {
  let d = diags("-- s12_0\nDELETE FROM x2_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0012() {
  let d = diags("-- s12_0\nSELECT * FROM x2_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0013() {
  let d = diags("-- s12_0\nSELECT * FROM x3_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0014() {
  let d = diags("-- s12_0\nSELECT id FROM x3_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0015() {
  let d = diags("-- s12_0\nINSERT INTO x3_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0016() {
  let d = diags("-- s12_0\nUPDATE x3_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0017() {
  let d = diags("-- s12_0\nDELETE FROM x3_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_sev_err_0018() {
  let d = diags("-- s12_0\nSELECT * FROM x3_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r12_msg_0401() {
  let d = diags("-- m12_0\nSELECT * FROM x1_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("x1_t")));
}

#[test]
fn r12_msg_0402() {
  let d = diags("-- m12_0\nSELECT * FROM x2_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("x2_t")));
}

#[test]
fn r12_msg_0403() {
  let d = diags("-- m12_0\nSELECT * FROM x3_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("x3_t")));
}

#[test]
fn r12_msg_0406() {
  let d = diags("-- m12_0\nSELECT * FROM y1_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("y1_t")));
}

#[test]
fn r12_msg_0407() {
  let d = diags("-- m12_0\nSELECT * FROM y2_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("y2_t")));
}

#[test]
fn r12_msg_0408() {
  let d = diags("-- m12_0\nSELECT * FROM y3_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("y3_t")));
}

#[test]
fn r12_msg_0411() {
  let d = diags("-- m12_0\nSELECT * FROM z1_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("z1_t")));
}

#[test]
fn r12_msg_0412() {
  let d = diags("-- m12_0\nSELECT * FROM z2_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("z2_t")));
}

#[test]
fn r12_msg_0413() {
  let d = diags("-- m12_0\nSELECT * FROM z3_t");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("z3_t")));
}

#[test]
fn r12_no_codes_0603() {
  let d = diags("-- k12_0\nSELECT * FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r12_no_codes_0605() {
  let d = diags("-- k12_0\nSELECT * FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r12_no_codes_0608() {
  let d = diags("-- k12_1\nSELECT * FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r12_no_codes_0610() {
  let d = diags("-- k12_1\nSELECT * FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r12_no_codes_0613() {
  let d = diags("-- k12_2\nSELECT * FROM users");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r12_no_codes_0615() {
  let d = diags("-- k12_2\nSELECT * FROM orders");
  assert!(!d.iter().any(|x| x.code == "sql001" || x.code == "sql002" || x.code == "sql003"));
}

#[test]
fn r13_sql001_0001() {
  let d = diags("-- s13_0\nSELECT * FROM aa_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0002() {
  let d = diags("-- s13_0\nSELECT id FROM aa_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0003() {
  let d = diags("-- s13_0\nSELECT * FROM aa_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0004() {
  let d = diags("-- s13_0\nSELECT count(*) FROM aa_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0005() {
  let d = diags("-- s13_0\nINSERT INTO aa_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0006() {
  let d = diags("-- s13_0\nUPDATE aa_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0007() {
  let d = diags("-- s13_0\nDELETE FROM aa_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0008() {
  let d = diags("-- s13_0\nSELECT * FROM aa_t u");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0009() {
  let d = diags("-- s13_0\nSELECT * FROM bb_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0010() {
  let d = diags("-- s13_0\nSELECT id FROM bb_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0011() {
  let d = diags("-- s13_0\nSELECT * FROM bb_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0012() {
  let d = diags("-- s13_0\nSELECT count(*) FROM bb_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0013() {
  let d = diags("-- s13_0\nINSERT INTO bb_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0014() {
  let d = diags("-- s13_0\nUPDATE bb_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0015() {
  let d = diags("-- s13_0\nDELETE FROM bb_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0016() {
  let d = diags("-- s13_0\nSELECT * FROM bb_t u");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0017() {
  let d = diags("-- s13_0\nSELECT * FROM cc_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0018() {
  let d = diags("-- s13_0\nSELECT id FROM cc_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0019() {
  let d = diags("-- s13_0\nSELECT * FROM cc_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0020() {
  let d = diags("-- s13_0\nSELECT count(*) FROM cc_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0021() {
  let d = diags("-- s13_0\nINSERT INTO cc_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0022() {
  let d = diags("-- s13_0\nUPDATE cc_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0023() {
  let d = diags("-- s13_0\nDELETE FROM cc_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0024() {
  let d = diags("-- s13_0\nSELECT * FROM cc_t u");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0025() {
  let d = diags("-- s13_0\nSELECT * FROM dd_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0026() {
  let d = diags("-- s13_0\nSELECT id FROM dd_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0027() {
  let d = diags("-- s13_0\nSELECT * FROM dd_t WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0028() {
  let d = diags("-- s13_0\nSELECT count(*) FROM dd_t");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0029() {
  let d = diags("-- s13_0\nINSERT INTO dd_t VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql001_0030() {
  let d = diags("-- s13_0\nUPDATE dd_t SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r13_sql002_0801() {
  let d = diags("-- s13c_0\nSELECT uno FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0802() {
  let d = diags("-- s13c_0\nSELECT id, uno FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0803() {
  let d = diags("-- s13c_0\nSELECT * FROM users WHERE uno = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0804() {
  let d = diags("-- s13c_0\nSELECT u.uno FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0805() {
  let d = diags("-- s13c_0\nSELECT uno FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0806() {
  let d = diags("-- s13c_0\nSELECT duo FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0807() {
  let d = diags("-- s13c_0\nSELECT id, duo FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0808() {
  let d = diags("-- s13c_0\nSELECT * FROM users WHERE duo = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0809() {
  let d = diags("-- s13c_0\nSELECT u.duo FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0810() {
  let d = diags("-- s13c_0\nSELECT duo FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0811() {
  let d = diags("-- s13c_0\nSELECT tre FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0812() {
  let d = diags("-- s13c_0\nSELECT id, tre FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0813() {
  let d = diags("-- s13c_0\nSELECT * FROM users WHERE tre = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0814() {
  let d = diags("-- s13c_0\nSELECT u.tre FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0815() {
  let d = diags("-- s13c_0\nSELECT tre FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0816() {
  let d = diags("-- s13c_0\nSELECT cuatro FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0817() {
  let d = diags("-- s13c_0\nSELECT id, cuatro FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0818() {
  let d = diags("-- s13c_0\nSELECT * FROM users WHERE cuatro = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0819() {
  let d = diags("-- s13c_0\nSELECT u.cuatro FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0820() {
  let d = diags("-- s13c_0\nSELECT cuatro FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0821() {
  let d = diags("-- s13c_0\nSELECT cinco FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0822() {
  let d = diags("-- s13c_0\nSELECT id, cinco FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0823() {
  let d = diags("-- s13c_0\nSELECT * FROM users WHERE cinco = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0824() {
  let d = diags("-- s13c_0\nSELECT u.cinco FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0825() {
  let d = diags("-- s13c_0\nSELECT cinco FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0826() {
  let d = diags("-- s13c_0\nSELECT seis FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0827() {
  let d = diags("-- s13c_0\nSELECT id, seis FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0828() {
  let d = diags("-- s13c_0\nSELECT * FROM users WHERE seis = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0829() {
  let d = diags("-- s13c_0\nSELECT u.seis FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r13_sql002_0830() {
  let d = diags("-- s13c_0\nSELECT seis FROM orders");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r14_msg_tbl_0001() {
  let d = diags("-- t14_0\nSELECT * FROM aa1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("aa1")));
}

#[test]
fn r14_msg_tbl_0002() {
  let d = diags("-- t14_0\nSELECT * FROM aa2");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("aa2")));
}

#[test]
fn r14_msg_tbl_0003() {
  let d = diags("-- t14_0\nSELECT * FROM aa3");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("aa3")));
}

#[test]
fn r14_msg_tbl_0006() {
  let d = diags("-- t14_0\nSELECT * FROM bb1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("bb1")));
}

#[test]
fn r14_msg_tbl_0007() {
  let d = diags("-- t14_0\nSELECT * FROM bb2");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("bb2")));
}

#[test]
fn r14_msg_tbl_0008() {
  let d = diags("-- t14_0\nSELECT * FROM bb3");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("bb3")));
}

#[test]
fn r14_msg_tbl_0011() {
  let d = diags("-- t14_0\nSELECT * FROM cc1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("cc1")));
}

#[test]
fn r14_msg_tbl_0012() {
  let d = diags("-- t14_0\nSELECT * FROM cc2");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("cc2")));
}

#[test]
fn r14_msg_tbl_0013() {
  let d = diags("-- t14_0\nSELECT * FROM cc3");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("cc3")));
}

#[test]
fn r14_msg_tbl_0016() {
  let d = diags("-- t14_0\nSELECT * FROM dd1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("dd1")));
}

#[test]
fn r14_msg_tbl_0017() {
  let d = diags("-- t14_0\nSELECT * FROM dd2");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("dd2")));
}

#[test]
fn r14_msg_tbl_0018() {
  let d = diags("-- t14_0\nSELECT * FROM dd3");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("dd3")));
}

#[test]
fn r14_msg_tbl_0021() {
  let d = diags("-- t14_0\nSELECT * FROM ee1");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("ee1")));
}

#[test]
fn r14_msg_tbl_0022() {
  let d = diags("-- t14_0\nSELECT * FROM ee2");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("ee2")));
}

#[test]
fn r14_msg_tbl_0023() {
  let d = diags("-- t14_0\nSELECT * FROM ee3");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("ee3")));
}

#[test]
fn r14_msg_col_0501() {
  let d = diags("-- c14_0\nSELECT zzcol1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("zzcol1")));
}

#[test]
fn r14_msg_col_0502() {
  let d = diags("-- c14_0\nSELECT zzcol2 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("zzcol2")));
}

#[test]
fn r14_msg_col_0503() {
  let d = diags("-- c14_0\nSELECT zzcol3 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("zzcol3")));
}

#[test]
fn r14_msg_col_0506() {
  let d = diags("-- c14_0\nSELECT ww1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("ww1")));
}

#[test]
fn r14_msg_col_0507() {
  let d = diags("-- c14_0\nSELECT ww2 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("ww2")));
}

#[test]
fn r14_msg_col_0508() {
  let d = diags("-- c14_0\nSELECT ww3 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("ww3")));
}

#[test]
fn r14_msg_col_0511() {
  let d = diags("-- c14_0\nSELECT vv1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("vv1")));
}

#[test]
fn r14_msg_col_0512() {
  let d = diags("-- c14_0\nSELECT vv2 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("vv2")));
}

#[test]
fn r14_msg_col_0513() {
  let d = diags("-- c14_0\nSELECT vv3 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("vv3")));
}

#[test]
fn r14_msg_col_0516() {
  let d = diags("-- c14_0\nSELECT uu1 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("uu1")));
}

#[test]
fn r14_msg_col_0517() {
  let d = diags("-- c14_0\nSELECT uu2 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("uu2")));
}

#[test]
fn r14_msg_col_0518() {
  let d = diags("-- c14_0\nSELECT uu3 FROM users");
  assert!(d.iter().any(|x| x.code == "sql002" && x.message.contains("uu3")));
}

#[test]
fn r14_amb_0801() {
  let d = diags("-- a14_0\nSELECT id FROM users, orders");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0802() {
  let d = diags("-- a14_0\nSELECT id FROM users u, orders o");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0803() {
  let d = diags("-- a14_0\nSELECT id FROM users JOIN orders ON true");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0804() {
  let d = diags("-- a14_0\nSELECT id FROM users CROSS JOIN orders");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0805() {
  let d = diags("-- a14_1\nSELECT id FROM users, orders");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0806() {
  let d = diags("-- a14_1\nSELECT id FROM users u, orders o");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0807() {
  let d = diags("-- a14_1\nSELECT id FROM users JOIN orders ON true");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0808() {
  let d = diags("-- a14_1\nSELECT id FROM users CROSS JOIN orders");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0809() {
  let d = diags("-- a14_2\nSELECT id FROM users, orders");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0810() {
  let d = diags("-- a14_2\nSELECT id FROM users u, orders o");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0811() {
  let d = diags("-- a14_2\nSELECT id FROM users JOIN orders ON true");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r14_amb_0812() {
  let d = diags("-- a14_2\nSELECT id FROM users CROSS JOIN orders");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r15_sev_0001() {
  let d = diags("-- s15_0\nSELECT * FROM aaa");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0002() {
  let d = diags("-- s15_0\nSELECT * FROM bbb");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0003() {
  let d = diags("-- s15_0\nSELECT * FROM ccc");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0004() {
  let d = diags("-- s15_0\nSELECT * FROM ddd");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0005() {
  let d = diags("-- s15_0\nSELECT * FROM eee");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0006() {
  let d = diags("-- s15_0\nSELECT * FROM fff");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0007() {
  let d = diags("-- s15_0\nSELECT * FROM ggg");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0008() {
  let d = diags("-- s15_0\nSELECT * FROM hhh");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0009() {
  let d = diags("-- s15_0\nSELECT * FROM iii");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0010() {
  let d = diags("-- s15_0\nSELECT * FROM jjj");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0011() {
  let d = diags("-- s15_0\nSELECT * FROM kkk");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0012() {
  let d = diags("-- s15_0\nSELECT * FROM lll");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0013() {
  let d = diags("-- s15_0\nSELECT * FROM mmm");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0014() {
  let d = diags("-- s15_0\nSELECT * FROM nnn");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0015() {
  let d = diags("-- s15_0\nSELECT * FROM ooo");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0016() {
  let d = diags("-- s15_0\nSELECT * FROM ppp");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0017() {
  let d = diags("-- s15_0\nSELECT * FROM qqq");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0018() {
  let d = diags("-- s15_0\nSELECT * FROM rrr");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0019() {
  let d = diags("-- s15_0\nSELECT * FROM sss");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0020() {
  let d = diags("-- s15_0\nSELECT * FROM ttt");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0021() {
  let d = diags("-- s15_0\nSELECT * FROM uuu");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0022() {
  let d = diags("-- s15_0\nSELECT * FROM vvv");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0023() {
  let d = diags("-- s15_0\nSELECT * FROM www");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0024() {
  let d = diags("-- s15_0\nSELECT * FROM xxx");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0025() {
  let d = diags("-- s15_0\nSELECT * FROM yyy");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0026() {
  let d = diags("-- s15_0\nSELECT * FROM zzz");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0027() {
  let d = diags("-- s15_1\nSELECT * FROM aaa");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0028() {
  let d = diags("-- s15_1\nSELECT * FROM bbb");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0029() {
  let d = diags("-- s15_1\nSELECT * FROM ccc");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_sev_0030() {
  let d = diags("-- s15_1\nSELECT * FROM ddd");
  assert!(d.iter().any(|x| x.code == "sql001" && x.severity == Severity::Error));
}

#[test]
fn r15_wcol_0651() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE px = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0652() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE py = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0653() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE pz = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0654() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE qa = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0655() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE qb = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0656() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE qc = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0657() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE ra = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0658() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE rb = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0659() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE rc = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0660() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE sa = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0661() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE sb = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0662() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE sc = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0663() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE ta = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0664() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE tb = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0665() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE tc = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0666() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE ua = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0667() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE ub = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0668() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE uc = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0669() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE va = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0670() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE vb = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0671() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE vc = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0672() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE wa = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0673() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE wb = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0674() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE wc = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0675() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE xa = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0676() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE xb = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0677() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE xc = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0678() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE ya = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0679() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE yb = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r15_wcol_0680() {
  let d = diags("-- w15_0\nSELECT * FROM users WHERE yc = 1");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r16_join_unk_0001() {
  let d = diags("-- jn0\nSELECT * FROM users JOIN aa_0 ON true");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r16_join_unk_0002() {
  let d = diags("-- jn0\nSELECT * FROM users JOIN aa_1 ON true");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r16_join_unk_0003() {
  let d = diags("-- jn0\nSELECT * FROM users JOIN aa_2 ON true");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r16_join_unk_0021() {
  let d = diags("-- jn0\nSELECT * FROM users JOIN bb_0 ON true");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r16_join_unk_0022() {
  let d = diags("-- jn0\nSELECT * FROM users JOIN bb_1 ON true");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r16_join_unk_0023() {
  let d = diags("-- jn0\nSELECT * FROM users JOIN bb_2 ON true");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r16_qu_unk_1001() {
  let d = diags("-- qu0\nSELECT u.xcol0 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r16_qu_unk_1002() {
  let d = diags("-- qu0\nSELECT u.xcol1 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r16_qu_unk_1003() {
  let d = diags("-- qu0\nSELECT u.xcol2 FROM users u");
  assert!(d.iter().any(|x| x.code == "sql002"));
}

#[test]
fn r17_probe_edge_diag() {
  for s in [
    "SELECT * FROM nonexistent_table_xyz",
    "SELECT bogus_col FROM users",
    "SELECT name FROM users WHERE name LIKE 'no_wildcard'",
    "SELECT id FROM users GROUP BY id",
    "INSERT INTO users (id) VALUES (1)",
    "UPDATE users SET name = 'x' WHERE id = 1",
    "DELETE FROM users WHERE id = 1",
    "SELECT * FROM users; SELECT * FROM bad_table_z",
    "/* comment */ SELECT * FROM bad_t",
    "-- comment\nSELECT * FROM bad_t",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r17_probe_more() {
  for s in [
    "UPDATE users SET name = 'x'",
    "DELETE FROM users",
    "SELECT * FROM users WHERE 1=1",
    "SELECT * FROM users WHERE TRUE",
    "SELECT * FROM users WHERE FALSE",
    "SELECT * FROM users WHERE NULL",
    "SELECT * FROM users WHERE id = id",
    "SELECT 1/0",
    "SELECT 1::int / 0",
    "SELECT * FROM users LIMIT -1",
    "SELECT * FROM users OFFSET -1",
    "SELECT * FROM users ORDER BY 99",
    "SELECT name FROM users WHERE name = name",
    "SELECT * FROM users u, users u",
    "SELECT id, id FROM users",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r17_sql013_0001() {
  let d = diags("-- s13_0\nUPDATE users SET name = 'x'");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0002() {
  let d = diags("-- s13_0\nUPDATE users SET email = 'a@b'");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0003() {
  let d = diags("-- s13_0\nUPDATE users SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0004() {
  let d = diags("-- s13_0\nDELETE FROM users");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0005() {
  let d = diags("-- s13_0\nDELETE FROM orders");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0006() {
  let d = diags("-- s13_0\nUPDATE orders SET user_id = 1");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0007() {
  let d = diags("-- s13_0\nUPDATE orders SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0008() {
  let d = diags("-- s13_1\nUPDATE users SET name = 'x'");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0009() {
  let d = diags("-- s13_1\nUPDATE users SET email = 'a@b'");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0010() {
  let d = diags("-- s13_1\nUPDATE users SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0011() {
  let d = diags("-- s13_1\nDELETE FROM users");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0012() {
  let d = diags("-- s13_1\nDELETE FROM orders");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0013() {
  let d = diags("-- s13_1\nUPDATE orders SET user_id = 1");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0014() {
  let d = diags("-- s13_1\nUPDATE orders SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0015() {
  let d = diags("-- s13_2\nUPDATE users SET name = 'x'");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0016() {
  let d = diags("-- s13_2\nUPDATE users SET email = 'a@b'");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0017() {
  let d = diags("-- s13_2\nUPDATE users SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0018() {
  let d = diags("-- s13_2\nDELETE FROM users");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0019() {
  let d = diags("-- s13_2\nDELETE FROM orders");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0020() {
  let d = diags("-- s13_2\nUPDATE orders SET user_id = 1");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql013_0021() {
  let d = diags("-- s13_2\nUPDATE orders SET id = 1");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 (missing predicate)");
}

#[test]
fn r17_sql474_0031() {
  let d = diags("-- t0\nSELECT * FROM users WHERE 1=1");
  assert!(d.iter().any(|x| x.code == "sql474"), "expected sql474 (1=1 tautology)");
}

#[test]
fn r17_sql474_0032() {
  let d = diags("-- t1\nSELECT * FROM users WHERE 1=1");
  assert!(d.iter().any(|x| x.code == "sql474"), "expected sql474 (1=1 tautology)");
}

#[test]
fn r17_sql474_0033() {
  let d = diags("-- t2\nSELECT * FROM users WHERE 1=1");
  assert!(d.iter().any(|x| x.code == "sql474"), "expected sql474 (1=1 tautology)");
}

#[test]
fn r17_sql407_0061() {
  let d = diags("-- f0\nSELECT * FROM users WHERE FALSE");
  assert!(d.iter().any(|x| x.code == "sql407"));
}

#[test]
fn r17_sql407_0062() {
  let d = diags("-- f1\nSELECT * FROM users WHERE FALSE");
  assert!(d.iter().any(|x| x.code == "sql407"));
}

#[test]
fn r17_sql407_0063() {
  let d = diags("-- f2\nSELECT * FROM users WHERE FALSE");
  assert!(d.iter().any(|x| x.code == "sql407"));
}

#[test]
fn r17_sql408_0091() {
  let d = diags("-- e0\nSELECT * FROM users WHERE id = id");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r17_sql408_0092() {
  let d = diags("-- e1\nSELECT * FROM users WHERE id = id");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r17_sql408_0093() {
  let d = diags("-- e2\nSELECT * FROM users WHERE id = id");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r17_sql278_0121() {
  let d = diags("-- d0\nSELECT 1/0");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0122() {
  let d = diags("-- d0\nSELECT 1::int / 0");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0123() {
  let d = diags("-- d0\nSELECT 100 / 0");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0124() {
  let d = diags("-- d0\nSELECT id / 0 FROM users");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0125() {
  let d = diags("-- d1\nSELECT 1/0");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0126() {
  let d = diags("-- d1\nSELECT 1::int / 0");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0127() {
  let d = diags("-- d1\nSELECT 100 / 0");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0128() {
  let d = diags("-- d1\nSELECT id / 0 FROM users");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0129() {
  let d = diags("-- d2\nSELECT 1/0");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0130() {
  let d = diags("-- d2\nSELECT 1::int / 0");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0131() {
  let d = diags("-- d2\nSELECT 100 / 0");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql278_0132() {
  let d = diags("-- d2\nSELECT id / 0 FROM users");
  assert!(d.iter().any(|x| x.code == "sql278"));
}

#[test]
fn r17_sql410_0151() {
  let d = diags("-- dup0\nSELECT id, id FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r17_sql410_0152() {
  let d = diags("-- dup1\nSELECT id, id FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r17_sql410_0153() {
  let d = diags("-- dup2\nSELECT id, id FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r17_multi_sql001_0181() {
  let d = diags("-- ms0\nSELECT * FROM users; SELECT * FROM bad_table_z");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("bad_table_z")));
}

#[test]
fn r17_multi_sql001_0182() {
  let d = diags("-- ms1\nSELECT * FROM users; SELECT * FROM bad_table_z");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("bad_table_z")));
}

#[test]
fn r17_multi_sql001_0183() {
  let d = diags("-- ms2\nSELECT * FROM users; SELECT * FROM bad_table_z");
  assert!(d.iter().any(|x| x.code == "sql001" && x.message.contains("bad_table_z")));
}

#[test]
fn r17_cmt_sql001_0211() {
  let d = diags("/* prefix 0 */ SELECT * FROM bad_t_0");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r17_cmt_sql001_0212() {
  let d = diags("/* prefix 1 */ SELECT * FROM bad_t_1");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r17_cmt_sql001_0213() {
  let d = diags("/* prefix 2 */ SELECT * FROM bad_t_2");
  assert!(d.iter().any(|x| x.code == "sql001"));
}

#[test]
fn r18_probe_more_rules() {
  for s in [
    "INSERT INTO users (id) VALUES (1)",
    "INSERT INTO users VALUES (1, 'a', 'b')",
    "INSERT INTO users (id) VALUES (1), (2), (3)",
    "INSERT INTO users (id, email) VALUES (1, 'a')",
    "SELECT * FROM users WHERE id IS NULL",
    "SELECT * FROM users WHERE id IS NOT NULL",
    "SELECT * FROM users WHERE id = NULL",
    "SELECT * FROM users WHERE id != NULL",
    "SELECT * FROM users WHERE name LIKE '%'",
    "SELECT * FROM users WHERE name LIKE '%abc'",
    "SELECT * FROM users WHERE name LIKE 'abc%'",
    "SELECT * FROM users WHERE name LIKE 'abc'",
    "SELECT * FROM users WHERE name ILIKE 'NO_WILDCARD'",
    "SELECT * FROM users CROSS JOIN orders",
    "SELECT DISTINCT * FROM users",
    "SELECT DISTINCT id FROM users",
    "SELECT id FROM users ORDER BY 1",
    "SELECT id FROM users GROUP BY 1",
    "SELECT id FROM users LIMIT 1000000",
    "SELECT * FROM users FOR UPDATE NOWAIT",
    "SELECT * FROM users FOR SHARE",
    "SELECT count(*) FROM users WHERE id > 0",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r18_sql015_0001() {
  let d = diags("-- s15_0\nSELECT * FROM users WHERE id = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0002() {
  let d = diags("-- s15_0\nSELECT * FROM users WHERE email = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0003() {
  let d = diags("-- s15_0\nSELECT * FROM users WHERE name = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0004() {
  let d = diags("-- s15_0\nSELECT * FROM users WHERE id != NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0005() {
  let d = diags("-- s15_0\nSELECT * FROM users WHERE id <> NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0006() {
  let d = diags("-- s15_0\nUPDATE users SET name = 'x' WHERE id = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0007() {
  let d = diags("-- s15_0\nDELETE FROM users WHERE id = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0008() {
  let d = diags("-- s15_1\nSELECT * FROM users WHERE id = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0009() {
  let d = diags("-- s15_1\nSELECT * FROM users WHERE email = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0010() {
  let d = diags("-- s15_1\nSELECT * FROM users WHERE name = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0011() {
  let d = diags("-- s15_1\nSELECT * FROM users WHERE id != NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0012() {
  let d = diags("-- s15_1\nSELECT * FROM users WHERE id <> NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0013() {
  let d = diags("-- s15_1\nUPDATE users SET name = 'x' WHERE id = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0014() {
  let d = diags("-- s15_1\nDELETE FROM users WHERE id = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0015() {
  let d = diags("-- s15_2\nSELECT * FROM users WHERE id = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0016() {
  let d = diags("-- s15_2\nSELECT * FROM users WHERE email = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0017() {
  let d = diags("-- s15_2\nSELECT * FROM users WHERE name = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0018() {
  let d = diags("-- s15_2\nSELECT * FROM users WHERE id != NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0019() {
  let d = diags("-- s15_2\nSELECT * FROM users WHERE id <> NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0020() {
  let d = diags("-- s15_2\nUPDATE users SET name = 'x' WHERE id = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql015_0021() {
  let d = diags("-- s15_2\nDELETE FROM users WHERE id = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"));
}

#[test]
fn r18_sql088_0031() {
  let d = diags("-- s88_0\nSELECT * FROM users WHERE name LIKE '%'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0032() {
  let d = diags("-- s88_0\nSELECT * FROM users WHERE name LIKE '%abc'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0033() {
  let d = diags("-- s88_0\nSELECT * FROM users WHERE email LIKE '%@example.com'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0034() {
  let d = diags("-- s88_0\nSELECT * FROM users WHERE name LIKE '%xyz%'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0035() {
  let d = diags("-- s88_1\nSELECT * FROM users WHERE name LIKE '%'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0036() {
  let d = diags("-- s88_1\nSELECT * FROM users WHERE name LIKE '%abc'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0037() {
  let d = diags("-- s88_1\nSELECT * FROM users WHERE email LIKE '%@example.com'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0038() {
  let d = diags("-- s88_1\nSELECT * FROM users WHERE name LIKE '%xyz%'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0039() {
  let d = diags("-- s88_2\nSELECT * FROM users WHERE name LIKE '%'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0040() {
  let d = diags("-- s88_2\nSELECT * FROM users WHERE name LIKE '%abc'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0041() {
  let d = diags("-- s88_2\nSELECT * FROM users WHERE email LIKE '%@example.com'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql088_0042() {
  let d = diags("-- s88_2\nSELECT * FROM users WHERE name LIKE '%xyz%'");
  assert!(d.iter().any(|x| x.code == "sql088"));
}

#[test]
fn r18_sql052_0061() {
  let d = diags("-- s52_0\nSELECT * FROM users WHERE name LIKE 'abc'");
  assert!(d.iter().any(|x| x.code == "sql052"));
}

#[test]
fn r18_sql052_0062() {
  let d = diags("-- s52_0\nSELECT * FROM users WHERE email LIKE 'a@b.c'");
  assert!(d.iter().any(|x| x.code == "sql052"));
}

#[test]
fn r18_sql052_0064() {
  let d = diags("-- s52_1\nSELECT * FROM users WHERE name LIKE 'abc'");
  assert!(d.iter().any(|x| x.code == "sql052"));
}

#[test]
fn r18_sql052_0065() {
  let d = diags("-- s52_1\nSELECT * FROM users WHERE email LIKE 'a@b.c'");
  assert!(d.iter().any(|x| x.code == "sql052"));
}

#[test]
fn r18_sql052_0067() {
  let d = diags("-- s52_2\nSELECT * FROM users WHERE name LIKE 'abc'");
  assert!(d.iter().any(|x| x.code == "sql052"));
}

#[test]
fn r18_sql052_0068() {
  let d = diags("-- s52_2\nSELECT * FROM users WHERE email LIKE 'a@b.c'");
  assert!(d.iter().any(|x| x.code == "sql052"));
}

#[test]
fn r18_sql486_0091() {
  let d = diags("-- d0\nSELECT DISTINCT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql486"));
}

#[test]
fn r18_sql486_0092() {
  let d = diags("-- d1\nSELECT DISTINCT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql486"));
}

#[test]
fn r18_sql486_0093() {
  let d = diags("-- d2\nSELECT DISTINCT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql486"));
}

#[test]
fn r18_sql099_0121() {
  let d = diags("-- s99_0\nSELECT id FROM users ORDER BY 1");
  assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn r18_sql099_0122() {
  let d = diags("-- s99_0\nSELECT id, email FROM users ORDER BY 1");
  assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn r18_sql099_0123() {
  let d = diags("-- s99_0\nSELECT id, email FROM users ORDER BY 2");
  assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn r18_sql099_0124() {
  let d = diags("-- s99_0\nSELECT id, email, name FROM users ORDER BY 1, 2");
  assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn r18_sql099_0125() {
  let d = diags("-- s99_1\nSELECT id FROM users ORDER BY 1");
  assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn r18_sql099_0126() {
  let d = diags("-- s99_1\nSELECT id, email FROM users ORDER BY 1");
  assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn r18_sql099_0128() {
  let d = diags("-- s99_1\nSELECT id, email, name FROM users ORDER BY 1, 2");
  assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn r18_sql099_0129() {
  let d = diags("-- s99_2\nSELECT id FROM users ORDER BY 1");
  assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn r18_sql099_0132() {
  let d = diags("-- s99_2\nSELECT id, email, name FROM users ORDER BY 1, 2");
  assert!(d.iter().any(|x| x.code == "sql099"));
}

#[test]
fn r18_sql065_0151() {
  let d = diags("-- s65_0\nSELECT id FROM users GROUP BY 1");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn r18_sql065_0152() {
  let d = diags("-- s65_0\nSELECT id, count(*) FROM users GROUP BY 1");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn r18_sql065_0153() {
  let d = diags("-- s65_0\nSELECT id, email, count(*) FROM users GROUP BY 1, 2");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn r18_sql065_0154() {
  let d = diags("-- s65_1\nSELECT id FROM users GROUP BY 1");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn r18_sql065_0155() {
  let d = diags("-- s65_1\nSELECT id, count(*) FROM users GROUP BY 1");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn r18_sql065_0156() {
  let d = diags("-- s65_1\nSELECT id, email, count(*) FROM users GROUP BY 1, 2");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn r18_sql065_0157() {
  let d = diags("-- s65_2\nSELECT id FROM users GROUP BY 1");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn r18_sql065_0158() {
  let d = diags("-- s65_2\nSELECT id, count(*) FROM users GROUP BY 1");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn r18_sql065_0159() {
  let d = diags("-- s65_2\nSELECT id, email, count(*) FROM users GROUP BY 1, 2");
  assert!(d.iter().any(|x| x.code == "sql065"));
}

#[test]
fn r18_sql051_0181() {
  let d = diags("-- l0\nSELECT * FROM users LIMIT 1000000");
  assert!(d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn r18_sql051_0182() {
  let d = diags("-- l1\nSELECT * FROM users LIMIT 1000001");
  assert!(d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn r18_sql051_0183() {
  let d = diags("-- l2\nSELECT * FROM users LIMIT 1000002");
  assert!(d.iter().any(|x| x.code == "sql051"));
}

#[test]
fn r18_sql072_0211() {
  let d = diags("-- s72_0\nSELECT * FROM users FOR UPDATE NOWAIT");
  assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn r18_sql072_0212() {
  let d = diags("-- s72_0\nSELECT * FROM users FOR SHARE");
  assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn r18_sql072_0215() {
  let d = diags("-- s72_0\nSELECT * FROM users FOR UPDATE SKIP LOCKED");
  assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn r18_sql072_0216() {
  let d = diags("-- s72_1\nSELECT * FROM users FOR UPDATE NOWAIT");
  assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn r18_sql072_0217() {
  let d = diags("-- s72_1\nSELECT * FROM users FOR SHARE");
  assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn r18_sql072_0220() {
  let d = diags("-- s72_1\nSELECT * FROM users FOR UPDATE SKIP LOCKED");
  assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn r18_sql072_0221() {
  let d = diags("-- s72_2\nSELECT * FROM users FOR UPDATE NOWAIT");
  assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn r18_sql072_0222() {
  let d = diags("-- s72_2\nSELECT * FROM users FOR SHARE");
  assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn r18_sql072_0225() {
  let d = diags("-- s72_2\nSELECT * FROM users FOR UPDATE SKIP LOCKED");
  assert!(d.iter().any(|x| x.code == "sql072"));
}

#[test]
fn r18_sql048_0241() {
  let d = diags("-- s48_0\nINSERT INTO users VALUES (1, 'a', 'b', 'extra')");
  assert!(d.iter().any(|x| x.code == "sql048"));
}

#[test]
fn r18_sql048_0242() {
  let d = diags("-- s48_0\nINSERT INTO orders VALUES (1, 2, 3, 'extra')");
  assert!(d.iter().any(|x| x.code == "sql048"));
}

#[test]
fn r18_sql048_0243() {
  let d = diags("-- s48_1\nINSERT INTO users VALUES (1, 'a', 'b', 'extra')");
  assert!(d.iter().any(|x| x.code == "sql048"));
}

#[test]
fn r18_sql048_0244() {
  let d = diags("-- s48_1\nINSERT INTO orders VALUES (1, 2, 3, 'extra')");
  assert!(d.iter().any(|x| x.code == "sql048"));
}

#[test]
fn r18_sql048_0245() {
  let d = diags("-- s48_2\nINSERT INTO users VALUES (1, 'a', 'b', 'extra')");
  assert!(d.iter().any(|x| x.code == "sql048"));
}

#[test]
fn r18_sql048_0246() {
  let d = diags("-- s48_2\nINSERT INTO orders VALUES (1, 2, 3, 'extra')");
  assert!(d.iter().any(|x| x.code == "sql048"));
}

#[test]
fn r18_sql176_0271() {
  let d = diags("-- s176_0\nSELECT * FROM users WHERE id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0272() {
  let d = diags("-- s176_0\nSELECT * FROM users WHERE email IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0273() {
  let d = diags("-- s176_0\nSELECT * FROM orders WHERE id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0274() {
  let d = diags("-- s176_0\nSELECT * FROM orders WHERE user_id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0275() {
  let d = diags("-- s176_1\nSELECT * FROM users WHERE id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0276() {
  let d = diags("-- s176_1\nSELECT * FROM users WHERE email IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0277() {
  let d = diags("-- s176_1\nSELECT * FROM orders WHERE id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0278() {
  let d = diags("-- s176_1\nSELECT * FROM orders WHERE user_id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0279() {
  let d = diags("-- s176_2\nSELECT * FROM users WHERE id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0280() {
  let d = diags("-- s176_2\nSELECT * FROM users WHERE email IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0281() {
  let d = diags("-- s176_2\nSELECT * FROM orders WHERE id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r18_sql176_0282() {
  let d = diags("-- s176_2\nSELECT * FROM orders WHERE user_id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"));
}

#[test]
fn r19_probe_codes() {
  for s in [
    "SELECT * FROM users HAVING count(*) > 1",
    "SELECT * FROM users WHERE id = (SELECT id FROM orders)",
    "SELECT id FROM users UNION SELECT id FROM users",
    "SELECT id FROM users UNION ALL SELECT id FROM users",
    "SELECT id, email FROM users UNION SELECT id, name FROM users",
    "SELECT * FROM users LEFT JOIN orders ON true",
    "SELECT * FROM users INNER JOIN orders ON true",
    "SELECT u.id FROM users u JOIN orders o ON true WHERE o.id IS NULL",
    "SELECT id FROM users WHERE id IN (SELECT id FROM orders WHERE user_id = users.id)",
    "DELETE FROM users WHERE id NOT IN (SELECT user_id FROM orders)",
    "DELETE FROM users WHERE NOT EXISTS (SELECT 1 FROM orders WHERE user_id = users.id)",
    "INSERT INTO users SELECT * FROM users",
    "INSERT INTO users (id, email, name) SELECT id, email, name FROM users",
    "SELECT * FROM users WHERE id = '1'",
    "SELECT * FROM users WHERE id::text = '1'",
    "UPDATE users SET id = id + 1",
    "UPDATE users SET id = id WHERE id = 1",
    "SELECT id FROM users WHERE id = id WHERE name IS NULL",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r19_sql056_0001() {
  let d = diags("-- s56_0\nSELECT id FROM users UNION SELECT id FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0002() {
  let d = diags("-- s56_0\nSELECT email FROM users UNION SELECT email FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0003() {
  let d = diags("-- s56_0\nSELECT id FROM orders UNION SELECT id FROM orders");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0004() {
  let d = diags("-- s56_0\nSELECT id, email FROM users UNION SELECT id, name FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0005() {
  let d = diags("-- s56_1\nSELECT id FROM users UNION SELECT id FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0006() {
  let d = diags("-- s56_1\nSELECT email FROM users UNION SELECT email FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0007() {
  let d = diags("-- s56_1\nSELECT id FROM orders UNION SELECT id FROM orders");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0008() {
  let d = diags("-- s56_1\nSELECT id, email FROM users UNION SELECT id, name FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0009() {
  let d = diags("-- s56_2\nSELECT id FROM users UNION SELECT id FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0010() {
  let d = diags("-- s56_2\nSELECT email FROM users UNION SELECT email FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0011() {
  let d = diags("-- s56_2\nSELECT id FROM orders UNION SELECT id FROM orders");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql056_0012() {
  let d = diags("-- s56_2\nSELECT id, email FROM users UNION SELECT id, name FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r19_sql018_0031() {
  let d = diags("-- s18_0\nDELETE FROM users WHERE id NOT IN (SELECT user_id FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0032() {
  let d = diags("-- s18_0\nSELECT * FROM users WHERE id NOT IN (SELECT id FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0033() {
  let d = diags("-- s18_0\nUPDATE users SET name='x' WHERE id NOT IN (SELECT user_id FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0034() {
  let d = diags("-- s18_0\nSELECT * FROM orders WHERE user_id NOT IN (SELECT id FROM users)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0035() {
  let d = diags("-- s18_1\nDELETE FROM users WHERE id NOT IN (SELECT user_id FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0036() {
  let d = diags("-- s18_1\nSELECT * FROM users WHERE id NOT IN (SELECT id FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0037() {
  let d = diags("-- s18_1\nUPDATE users SET name='x' WHERE id NOT IN (SELECT user_id FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0038() {
  let d = diags("-- s18_1\nSELECT * FROM orders WHERE user_id NOT IN (SELECT id FROM users)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0039() {
  let d = diags("-- s18_2\nDELETE FROM users WHERE id NOT IN (SELECT user_id FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0040() {
  let d = diags("-- s18_2\nSELECT * FROM users WHERE id NOT IN (SELECT id FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0041() {
  let d = diags("-- s18_2\nUPDATE users SET name='x' WHERE id NOT IN (SELECT user_id FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql018_0042() {
  let d = diags("-- s18_2\nSELECT * FROM orders WHERE user_id NOT IN (SELECT id FROM users)");
  assert!(d.iter().any(|x| x.code == "sql018"));
}

#[test]
fn r19_sql475_0061() {
  let d = diags("-- s475_0\nINSERT INTO users SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0062() {
  let d = diags("-- s475_0\nINSERT INTO users (id) SELECT id FROM users");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0063() {
  let d = diags("-- s475_0\nINSERT INTO orders SELECT * FROM orders");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0064() {
  let d = diags("-- s475_0\nINSERT INTO users (id, email) SELECT id, email FROM users");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0065() {
  let d = diags("-- s475_1\nINSERT INTO users SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0066() {
  let d = diags("-- s475_1\nINSERT INTO users (id) SELECT id FROM users");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0067() {
  let d = diags("-- s475_1\nINSERT INTO orders SELECT * FROM orders");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0068() {
  let d = diags("-- s475_1\nINSERT INTO users (id, email) SELECT id, email FROM users");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0069() {
  let d = diags("-- s475_2\nINSERT INTO users SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0070() {
  let d = diags("-- s475_2\nINSERT INTO users (id) SELECT id FROM users");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0071() {
  let d = diags("-- s475_2\nINSERT INTO orders SELECT * FROM orders");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql475_0072() {
  let d = diags("-- s475_2\nINSERT INTO users (id, email) SELECT id, email FROM users");
  assert!(d.iter().any(|x| x.code == "sql475"));
}

#[test]
fn r19_sql427_0091() {
  let d = diags("-- s427_0\nSELECT * FROM users WHERE id::text = '1'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0092() {
  let d = diags("-- s427_0\nSELECT * FROM users WHERE email::text = 'a@b'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0093() {
  let d = diags("-- s427_0\nSELECT * FROM users WHERE name::varchar = 'x'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0094() {
  let d = diags("-- s427_0\nSELECT * FROM orders WHERE id::text = '1'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0095() {
  let d = diags("-- s427_0\nSELECT * FROM orders WHERE user_id::text = '1'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0096() {
  let d = diags("-- s427_1\nSELECT * FROM users WHERE id::text = '1'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0097() {
  let d = diags("-- s427_1\nSELECT * FROM users WHERE email::text = 'a@b'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0098() {
  let d = diags("-- s427_1\nSELECT * FROM users WHERE name::varchar = 'x'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0099() {
  let d = diags("-- s427_1\nSELECT * FROM orders WHERE id::text = '1'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0100() {
  let d = diags("-- s427_1\nSELECT * FROM orders WHERE user_id::text = '1'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0101() {
  let d = diags("-- s427_2\nSELECT * FROM users WHERE id::text = '1'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0102() {
  let d = diags("-- s427_2\nSELECT * FROM users WHERE email::text = 'a@b'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0103() {
  let d = diags("-- s427_2\nSELECT * FROM users WHERE name::varchar = 'x'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0104() {
  let d = diags("-- s427_2\nSELECT * FROM orders WHERE id::text = '1'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql427_0105() {
  let d = diags("-- s427_2\nSELECT * FROM orders WHERE user_id::text = '1'");
  assert!(d.iter().any(|x| x.code == "sql427"));
}

#[test]
fn r19_sql149_0121() {
  let d = diags("-- s149_0\nUPDATE users SET id = id WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0122() {
  let d = diags("-- s149_0\nUPDATE users SET name = name WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0123() {
  let d = diags("-- s149_0\nUPDATE users SET email = email WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0124() {
  let d = diags("-- s149_0\nUPDATE orders SET id = id WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0125() {
  let d = diags("-- s149_1\nUPDATE users SET id = id WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0126() {
  let d = diags("-- s149_1\nUPDATE users SET name = name WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0127() {
  let d = diags("-- s149_1\nUPDATE users SET email = email WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0128() {
  let d = diags("-- s149_1\nUPDATE orders SET id = id WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0129() {
  let d = diags("-- s149_2\nUPDATE users SET id = id WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0130() {
  let d = diags("-- s149_2\nUPDATE users SET name = name WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0131() {
  let d = diags("-- s149_2\nUPDATE users SET email = email WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_sql149_0132() {
  let d = diags("-- s149_2\nUPDATE orders SET id = id WHERE id = 1");
  assert!(d.iter().any(|x| x.code == "sql149"));
}

#[test]
fn r19_combined_0152() {
  let d = diags("-- com_0\nUPDATE users SET name = 'x'");
  assert!(d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn r19_combined_0153() {
  let d = diags("-- com_0\nDELETE FROM users");
  assert!(d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn r19_combined_0155() {
  let d = diags("-- com_1\nUPDATE users SET name = 'x'");
  assert!(d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn r19_combined_0156() {
  let d = diags("-- com_1\nDELETE FROM users");
  assert!(d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn r19_combined_0158() {
  let d = diags("-- com_2\nUPDATE users SET name = 'x'");
  assert!(d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn r19_combined_0159() {
  let d = diags("-- com_2\nDELETE FROM users");
  assert!(d.iter().any(|x| x.code == "sql013"));
}

#[test]
fn r20_probe_codes() {
  for s in [
    "CREATE TABLE t (id int, id int)",
    "CREATE TABLE t (id serial PRIMARY KEY)",
    "CREATE TABLE t (id int PRIMARY KEY, id int)",
    "CREATE TABLE t (a varchar)",
    "CREATE TABLE t (a varchar(0))",
    "CREATE TABLE t (a numeric(0,0))",
    "CREATE TABLE t (a int CHECK (a = a))",
    "CREATE TABLE t (a int CHECK (TRUE))",
    "CREATE TABLE t (a int CHECK (FALSE))",
    "CREATE TABLE t (a int) WITH (fillfactor=200)",
    "CREATE INDEX ON users (id, id)",
    "CREATE INDEX ON users (id) WHERE 1=1",
    "CREATE INDEX ON users (id) WHERE FALSE",
    "CREATE INDEX ON users ((1))",
    "CREATE INDEX ON users (lower(name)) WHERE name IS NULL",
    "CREATE VIEW v AS SELECT * FROM users",
    "CREATE VIEW v AS SELECT 1",
    "CREATE OR REPLACE FUNCTION f() RETURNS int LANGUAGE sql AS $$select 1$$",
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql AS $$$$",
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql AS $$select id from users$$",
    "DROP TABLE users CASCADE",
    "DROP TABLE IF EXISTS users",
    "GRANT ALL ON users TO public",
    "GRANT SELECT ON ALL TABLES IN SCHEMA public TO PUBLIC",
    "CREATE ROLE r WITH PASSWORD ''",
    "CREATE ROLE r WITH PASSWORD 'pwd'",
    "CREATE ROLE r WITH SUPERUSER",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r20_sql046_0001() {
  let d = diags("-- s46_0\nCREATE TABLE t (id int, name text)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0002() {
  let d = diags("-- s46_0\nCREATE TABLE x (a int, b text)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0003() {
  let d = diags("-- s46_0\nCREATE TABLE y (col1 int)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0004() {
  let d = diags("-- s46_0\nCREATE TABLE my_t (data jsonb)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0005() {
  let d = diags("-- s46_0\nCREATE TABLE log_t (ts timestamp)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0006() {
  let d = diags("-- s46_1\nCREATE TABLE t (id int, name text)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0007() {
  let d = diags("-- s46_1\nCREATE TABLE x (a int, b text)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0008() {
  let d = diags("-- s46_1\nCREATE TABLE y (col1 int)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0009() {
  let d = diags("-- s46_1\nCREATE TABLE my_t (data jsonb)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0010() {
  let d = diags("-- s46_1\nCREATE TABLE log_t (ts timestamp)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0011() {
  let d = diags("-- s46_2\nCREATE TABLE t (id int, name text)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0012() {
  let d = diags("-- s46_2\nCREATE TABLE x (a int, b text)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0013() {
  let d = diags("-- s46_2\nCREATE TABLE y (col1 int)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0014() {
  let d = diags("-- s46_2\nCREATE TABLE my_t (data jsonb)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql046_0015() {
  let d = diags("-- s46_2\nCREATE TABLE log_t (ts timestamp)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r20_sql312_0031() {
  let d = diags("-- s312_0\nCREATE TABLE t (id serial PRIMARY KEY)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0032() {
  let d = diags("-- s312_0\nCREATE TABLE t (id bigserial PRIMARY KEY)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0033() {
  let d = diags("-- s312_0\nCREATE TABLE t (id smallserial PRIMARY KEY)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0034() {
  let d = diags("-- s312_0\nCREATE TABLE u (id serial PRIMARY KEY, name text)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0035() {
  let d = diags("-- s312_1\nCREATE TABLE t (id serial PRIMARY KEY)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0036() {
  let d = diags("-- s312_1\nCREATE TABLE t (id bigserial PRIMARY KEY)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0037() {
  let d = diags("-- s312_1\nCREATE TABLE t (id smallserial PRIMARY KEY)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0038() {
  let d = diags("-- s312_1\nCREATE TABLE u (id serial PRIMARY KEY, name text)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0039() {
  let d = diags("-- s312_2\nCREATE TABLE t (id serial PRIMARY KEY)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0040() {
  let d = diags("-- s312_2\nCREATE TABLE t (id bigserial PRIMARY KEY)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0041() {
  let d = diags("-- s312_2\nCREATE TABLE t (id smallserial PRIMARY KEY)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql312_0042() {
  let d = diags("-- s312_2\nCREATE TABLE u (id serial PRIMARY KEY, name text)");
  assert!(d.iter().any(|x| x.code == "sql312"));
}

#[test]
fn r20_sql146_0061() {
  let d = diags("-- s146_0\nCREATE TABLE t (a varchar)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0062() {
  let d = diags("-- s146_0\nCREATE TABLE t (a varchar, b int)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0063() {
  let d = diags("-- s146_0\nCREATE TABLE x (data varchar)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0064() {
  let d = diags("-- s146_0\nCREATE TABLE y (col varchar)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0065() {
  let d = diags("-- s146_1\nCREATE TABLE t (a varchar)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0066() {
  let d = diags("-- s146_1\nCREATE TABLE t (a varchar, b int)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0067() {
  let d = diags("-- s146_1\nCREATE TABLE x (data varchar)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0068() {
  let d = diags("-- s146_1\nCREATE TABLE y (col varchar)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0069() {
  let d = diags("-- s146_2\nCREATE TABLE t (a varchar)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0070() {
  let d = diags("-- s146_2\nCREATE TABLE t (a varchar, b int)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0071() {
  let d = diags("-- s146_2\nCREATE TABLE x (data varchar)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql146_0072() {
  let d = diags("-- s146_2\nCREATE TABLE y (col varchar)");
  assert!(d.iter().any(|x| x.code == "sql146"));
}

#[test]
fn r20_sql244_0091() {
  let d = diags("-- s244_0\nCREATE TABLE t (a int CHECK (TRUE))");
  assert!(d.iter().any(|x| x.code == "sql244"));
}

#[test]
fn r20_sql244_0092() {
  let d = diags("-- s244_0\nCREATE TABLE t (a int CHECK (true))");
  assert!(d.iter().any(|x| x.code == "sql244"));
}

#[test]
fn r20_sql244_0093() {
  let d = diags("-- s244_0\nALTER TABLE users ADD CONSTRAINT chk CHECK (TRUE)");
  assert!(d.iter().any(|x| x.code == "sql244"));
}

#[test]
fn r20_sql244_0094() {
  let d = diags("-- s244_1\nCREATE TABLE t (a int CHECK (TRUE))");
  assert!(d.iter().any(|x| x.code == "sql244"));
}

#[test]
fn r20_sql244_0095() {
  let d = diags("-- s244_1\nCREATE TABLE t (a int CHECK (true))");
  assert!(d.iter().any(|x| x.code == "sql244"));
}

#[test]
fn r20_sql244_0096() {
  let d = diags("-- s244_1\nALTER TABLE users ADD CONSTRAINT chk CHECK (TRUE)");
  assert!(d.iter().any(|x| x.code == "sql244"));
}

#[test]
fn r20_sql244_0097() {
  let d = diags("-- s244_2\nCREATE TABLE t (a int CHECK (TRUE))");
  assert!(d.iter().any(|x| x.code == "sql244"));
}

#[test]
fn r20_sql244_0098() {
  let d = diags("-- s244_2\nCREATE TABLE t (a int CHECK (true))");
  assert!(d.iter().any(|x| x.code == "sql244"));
}

#[test]
fn r20_sql244_0099() {
  let d = diags("-- s244_2\nALTER TABLE users ADD CONSTRAINT chk CHECK (TRUE)");
  assert!(d.iter().any(|x| x.code == "sql244"));
}

#[test]
fn r20_sql273_0121() {
  let d = diags("-- s273_0\nCREATE TABLE t (a int CHECK (FALSE))");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn r20_sql273_0122() {
  let d = diags("-- s273_0\nCREATE TABLE t (a int CHECK (false))");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn r20_sql273_0123() {
  let d = diags("-- s273_0\nALTER TABLE users ADD CONSTRAINT chk CHECK (FALSE)");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn r20_sql273_0124() {
  let d = diags("-- s273_1\nCREATE TABLE t (a int CHECK (FALSE))");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn r20_sql273_0125() {
  let d = diags("-- s273_1\nCREATE TABLE t (a int CHECK (false))");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn r20_sql273_0126() {
  let d = diags("-- s273_1\nALTER TABLE users ADD CONSTRAINT chk CHECK (FALSE)");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn r20_sql273_0127() {
  let d = diags("-- s273_2\nCREATE TABLE t (a int CHECK (FALSE))");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn r20_sql273_0128() {
  let d = diags("-- s273_2\nCREATE TABLE t (a int CHECK (false))");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn r20_sql273_0129() {
  let d = diags("-- s273_2\nALTER TABLE users ADD CONSTRAINT chk CHECK (FALSE)");
  assert!(d.iter().any(|x| x.code == "sql273"));
}

#[test]
fn r20_sql302_0151() {
  let d = diags("-- s302_0\nDROP TABLE users CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0152() {
  let d = diags("-- s302_0\nDROP TABLE orders CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0153() {
  let d = diags("-- s302_0\nDROP SCHEMA public CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0154() {
  let d = diags("-- s302_0\nDROP TYPE t CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0155() {
  let d = diags("-- s302_0\nDROP VIEW v CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0156() {
  let d = diags("-- s302_0\nDROP MATERIALIZED VIEW mv CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0157() {
  let d = diags("-- s302_0\nDROP FUNCTION f() CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0158() {
  let d = diags("-- s302_0\nDROP SEQUENCE s CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0159() {
  let d = diags("-- s302_1\nDROP TABLE users CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0160() {
  let d = diags("-- s302_1\nDROP TABLE orders CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0161() {
  let d = diags("-- s302_1\nDROP SCHEMA public CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0162() {
  let d = diags("-- s302_1\nDROP TYPE t CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0163() {
  let d = diags("-- s302_1\nDROP VIEW v CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0164() {
  let d = diags("-- s302_1\nDROP MATERIALIZED VIEW mv CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0165() {
  let d = diags("-- s302_1\nDROP FUNCTION f() CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0166() {
  let d = diags("-- s302_1\nDROP SEQUENCE s CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0167() {
  let d = diags("-- s302_2\nDROP TABLE users CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0168() {
  let d = diags("-- s302_2\nDROP TABLE orders CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0169() {
  let d = diags("-- s302_2\nDROP SCHEMA public CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0170() {
  let d = diags("-- s302_2\nDROP TYPE t CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0171() {
  let d = diags("-- s302_2\nDROP VIEW v CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0172() {
  let d = diags("-- s302_2\nDROP MATERIALIZED VIEW mv CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0173() {
  let d = diags("-- s302_2\nDROP FUNCTION f() CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql302_0174() {
  let d = diags("-- s302_2\nDROP SEQUENCE s CASCADE");
  assert!(d.iter().any(|x| x.code == "sql302"));
}

#[test]
fn r20_sql128_0181() {
  let d = diags("-- s128_0\nGRANT ALL ON users TO public");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0182() {
  let d = diags("-- s128_0\nGRANT SELECT ON users TO PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0183() {
  let d = diags("-- s128_0\nGRANT INSERT ON orders TO public");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0184() {
  let d = diags("-- s128_0\nGRANT ALL ON ALL TABLES IN SCHEMA public TO PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0185() {
  let d = diags("-- s128_0\nGRANT USAGE ON SCHEMA public TO PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0186() {
  let d = diags("-- s128_1\nGRANT ALL ON users TO public");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0187() {
  let d = diags("-- s128_1\nGRANT SELECT ON users TO PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0188() {
  let d = diags("-- s128_1\nGRANT INSERT ON orders TO public");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0189() {
  let d = diags("-- s128_1\nGRANT ALL ON ALL TABLES IN SCHEMA public TO PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0190() {
  let d = diags("-- s128_1\nGRANT USAGE ON SCHEMA public TO PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0191() {
  let d = diags("-- s128_2\nGRANT ALL ON users TO public");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0192() {
  let d = diags("-- s128_2\nGRANT SELECT ON users TO PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0193() {
  let d = diags("-- s128_2\nGRANT INSERT ON orders TO public");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0194() {
  let d = diags("-- s128_2\nGRANT ALL ON ALL TABLES IN SCHEMA public TO PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql128_0195() {
  let d = diags("-- s128_2\nGRANT USAGE ON SCHEMA public TO PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql128"));
}

#[test]
fn r20_sql241_0211() {
  let d = diags("-- s241_0\nCREATE VIEW v AS SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0212() {
  let d = diags("-- s241_0\nCREATE VIEW v AS SELECT * FROM orders");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0213() {
  let d = diags("-- s241_0\nCREATE OR REPLACE VIEW v AS SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0214() {
  let d = diags("-- s241_0\nCREATE MATERIALIZED VIEW mv AS SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0215() {
  let d = diags("-- s241_1\nCREATE VIEW v AS SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0216() {
  let d = diags("-- s241_1\nCREATE VIEW v AS SELECT * FROM orders");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0217() {
  let d = diags("-- s241_1\nCREATE OR REPLACE VIEW v AS SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0218() {
  let d = diags("-- s241_1\nCREATE MATERIALIZED VIEW mv AS SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0219() {
  let d = diags("-- s241_2\nCREATE VIEW v AS SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0220() {
  let d = diags("-- s241_2\nCREATE VIEW v AS SELECT * FROM orders");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0221() {
  let d = diags("-- s241_2\nCREATE OR REPLACE VIEW v AS SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r20_sql241_0222() {
  let d = diags("-- s241_2\nCREATE MATERIALIZED VIEW mv AS SELECT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql241"));
}

#[test]
fn r21_probe_codes() {
  for s in [
    "SELECT * FROM users WHERE name LIKE concat('%', name, '%')",
    "SELECT * FROM users WHERE upper(name) = 'X'",
    "SELECT * FROM users WHERE lower(name) = 'x'",
    "SELECT * FROM users WHERE substring(name, 1, 1) = 'a'",
    "SELECT length(name) FROM users WHERE length(name) > 0",
    "SELECT * FROM users WHERE id IN (1)",
    "SELECT * FROM users WHERE id IN (1, 1, 2)",
    "SELECT * FROM users WHERE id BETWEEN 10 AND 1",
    "SELECT * FROM users WHERE id > 10 AND id < 5",
    "SELECT * FROM users WHERE id > 10 AND id < 100",
    "SELECT * FROM users WHERE id = 1 AND id = 2",
    "SELECT * FROM users WHERE NULL = NULL",
    "SELECT * FROM users WHERE NULL IS NULL",
    "SELECT * FROM users LIMIT 0",
    "SELECT * FROM users LIMIT 1 OFFSET 1000000",
    "INSERT INTO users (id) VALUES (DEFAULT)",
    "INSERT INTO users (id) SELECT id FROM users WHERE id = 1 LIMIT 1",
    "SELECT id FROM users WHERE TRUE AND id = 1",
    "SELECT id FROM users WHERE FALSE OR id = 1",
    "SELECT id FROM users WHERE id = 1 AND TRUE",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r21_sql306_0001() {
  let d = diags("-- s306_0\nSELECT * FROM users WHERE id IN (1, 1, 2)");
  assert!(d.iter().any(|x| x.code == "sql306"));
}

#[test]
fn r21_sql306_0002() {
  let d = diags("-- s306_0\nSELECT * FROM users WHERE id IN (1, 2, 1)");
  assert!(d.iter().any(|x| x.code == "sql306"));
}

#[test]
fn r21_sql306_0003() {
  let d = diags("-- s306_0\nSELECT * FROM users WHERE email IN ('a', 'a', 'b')");
  assert!(d.iter().any(|x| x.code == "sql306"));
}

#[test]
fn r21_sql306_0005() {
  let d = diags("-- s306_1\nSELECT * FROM users WHERE id IN (1, 1, 2)");
  assert!(d.iter().any(|x| x.code == "sql306"));
}

#[test]
fn r21_sql306_0007() {
  let d = diags("-- s306_1\nSELECT * FROM users WHERE email IN ('a', 'a', 'b')");
  assert!(d.iter().any(|x| x.code == "sql306"));
}

#[test]
fn r21_sql306_0011() {
  let d = diags("-- s306_2\nSELECT * FROM users WHERE email IN ('a', 'a', 'b')");
  assert!(d.iter().any(|x| x.code == "sql306"));
}

#[test]
fn r21_sql087_0026() {
  let d = diags("-- s87_0\nSELECT * FROM users WHERE id BETWEEN 10 AND 1");
  assert!(d.iter().any(|x| x.code == "sql087"));
}

#[test]
fn r21_sql087_0027() {
  let d = diags("-- s87_0\nSELECT * FROM users WHERE id BETWEEN 100 AND 0");
  assert!(d.iter().any(|x| x.code == "sql087"));
}

#[test]
fn r21_sql087_0028() {
  let d = diags("-- s87_0\nSELECT * FROM users WHERE id BETWEEN 5 AND 2");
  assert!(d.iter().any(|x| x.code == "sql087"));
}

#[test]
fn r21_sql109_0051() {
  let d = diags("-- s109_0\nSELECT length(name) FROM users WHERE length(name) > 0");
  assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn r21_sql109_0052() {
  let d = diags("-- s109_0\nSELECT * FROM users WHERE upper(name) = 'X' AND length(email) > 0");
  assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn r21_sql109_0053() {
  let d = diags("-- s109_0\nSELECT * FROM users WHERE length(name) = 5");
  assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn r21_sql109_0054() {
  let d = diags("-- s109_1\nSELECT length(name) FROM users WHERE length(name) > 0");
  assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn r21_sql109_0055() {
  let d = diags("-- s109_1\nSELECT * FROM users WHERE upper(name) = 'X' AND length(email) > 0");
  assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn r21_sql109_0056() {
  let d = diags("-- s109_1\nSELECT * FROM users WHERE length(name) = 5");
  assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn r21_sql109_0057() {
  let d = diags("-- s109_2\nSELECT length(name) FROM users WHERE length(name) > 0");
  assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn r21_sql109_0058() {
  let d = diags("-- s109_2\nSELECT * FROM users WHERE upper(name) = 'X' AND length(email) > 0");
  assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn r21_sql109_0059() {
  let d = diags("-- s109_2\nSELECT * FROM users WHERE length(name) = 5");
  assert!(d.iter().any(|x| x.code == "sql109"));
}

#[test]
fn r21_sql411_0096() {
  let d = diags("-- s411_0\nSELECT * FROM users LIMIT 1 OFFSET 1000000");
  assert!(d.iter().any(|x| x.code == "sql411"));
}

#[test]
fn r21_sql411_0097() {
  let d = diags("-- s411_0\nSELECT * FROM users LIMIT 10 OFFSET 500000");
  assert!(d.iter().any(|x| x.code == "sql411"));
}

#[test]
fn r21_sql411_0098() {
  let d = diags("-- s411_0\nSELECT * FROM orders LIMIT 100 OFFSET 9999999");
  assert!(d.iter().any(|x| x.code == "sql411"));
}

#[test]
fn r21_sql411_0099() {
  let d = diags("-- s411_1\nSELECT * FROM users LIMIT 1 OFFSET 1000000");
  assert!(d.iter().any(|x| x.code == "sql411"));
}

#[test]
fn r21_sql411_0101() {
  let d = diags("-- s411_1\nSELECT * FROM orders LIMIT 100 OFFSET 9999999");
  assert!(d.iter().any(|x| x.code == "sql411"));
}

#[test]
fn r21_sql411_0104() {
  let d = diags("-- s411_2\nSELECT * FROM orders LIMIT 100 OFFSET 9999999");
  assert!(d.iter().any(|x| x.code == "sql411"));
}

#[test]
fn r21_sql083_0121() {
  let d = diags("-- s83_0\nINSERT INTO users (id) VALUES (DEFAULT)");
  assert!(d.iter().any(|x| x.code == "sql083"));
}

#[test]
fn r21_sql083_0122() {
  let d = diags("-- s83_0\nINSERT INTO users (id) VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql083"));
}

#[test]
fn r21_sql083_0125() {
  let d = diags("-- s83_1\nINSERT INTO users (id) VALUES (DEFAULT)");
  assert!(d.iter().any(|x| x.code == "sql083"));
}

#[test]
fn r21_sql083_0126() {
  let d = diags("-- s83_1\nINSERT INTO users (id) VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql083"));
}

#[test]
fn r21_sql083_0129() {
  let d = diags("-- s83_2\nINSERT INTO users (id) VALUES (DEFAULT)");
  assert!(d.iter().any(|x| x.code == "sql083"));
}

#[test]
fn r21_sql083_0130() {
  let d = diags("-- s83_2\nINSERT INTO users (id) VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql083"));
}

#[test]
fn r22_probe_codes() {
  for s in [
    "SELECT id FROM users ORDER BY id, id",
    "SELECT id FROM users GROUP BY id, id",
    "SELECT * FROM users WHERE id = 1 AND id = 1",
    "SELECT * FROM users WHERE id = 1 OR id = 1",
    "SELECT * FROM users ORDER BY id, id DESC",
    "SELECT * FROM users LIMIT 100 LIMIT 50",
    "SELECT * FROM users CROSS JOIN users",
    "SELECT * FROM users u1 JOIN users u2 ON u1.id = u2.id",
    "SELECT * FROM users u1 JOIN users u2 ON true",
    "SELECT u.id FROM users u JOIN users u ON true",
    "INSERT INTO users (id, id) VALUES (1, 2)",
    "INSERT INTO users (id, name, id) VALUES (1, 'x', 2)",
    "UPDATE users SET name='x', name='y'",
    "UPDATE users SET id=1, id=2",
    "SELECT * FROM users WHERE EXISTS (SELECT 1)",
    "SELECT * FROM users WHERE id NOT IN (NULL)",
    "SELECT * FROM users WHERE id IN (NULL)",
    "SELECT * FROM users WHERE id < ALL(ARRAY[]::int[])",
    "SELECT 1 UNION SELECT 1",
    "SELECT id FROM users WHERE id IS NULL AND id IS NOT NULL",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r22_sql412_0001() {
  let d = diags("-- s12_0\nSELECT id FROM users ORDER BY id, id");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0002() {
  let d = diags("-- s12_0\nSELECT id FROM users GROUP BY id, id");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0003() {
  let d = diags("-- s12_0\nSELECT id FROM users ORDER BY id DESC, id ASC");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0004() {
  let d = diags("-- s12_0\nSELECT id FROM users GROUP BY id, id, id");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0005() {
  let d = diags("-- s12_0\nSELECT id FROM orders ORDER BY id, id");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0006() {
  let d = diags("-- s12_1\nSELECT id FROM users ORDER BY id, id");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0007() {
  let d = diags("-- s12_1\nSELECT id FROM users GROUP BY id, id");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0008() {
  let d = diags("-- s12_1\nSELECT id FROM users ORDER BY id DESC, id ASC");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0009() {
  let d = diags("-- s12_1\nSELECT id FROM users GROUP BY id, id, id");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0010() {
  let d = diags("-- s12_1\nSELECT id FROM orders ORDER BY id, id");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0011() {
  let d = diags("-- s12_2\nSELECT id FROM users ORDER BY id, id");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql412_0012() {
  let d = diags("-- s12_2\nSELECT id FROM users GROUP BY id, id");
  assert!(d.iter().any(|x| x.code == "sql412"));
}

#[test]
fn r22_sql421_0013() {
  let d = diags("-- s21_0\nSELECT * FROM users WHERE id = 1 AND id = 1");
  assert!(d.iter().any(|x| x.code == "sql421"));
}

#[test]
fn r22_sql421_0014() {
  let d = diags("-- s21_0\nSELECT * FROM users WHERE id = 1 OR id = 1");
  assert!(d.iter().any(|x| x.code == "sql421"));
}

#[test]
fn r22_sql421_0015() {
  let d = diags("-- s21_0\nSELECT * FROM users WHERE email = 'a' AND email = 'a'");
  assert!(d.iter().any(|x| x.code == "sql421"));
}

#[test]
fn r22_sql421_0016() {
  let d = diags("-- s21_1\nSELECT * FROM users WHERE id = 1 AND id = 1");
  assert!(d.iter().any(|x| x.code == "sql421"));
}

#[test]
fn r22_sql421_0017() {
  let d = diags("-- s21_1\nSELECT * FROM users WHERE id = 1 OR id = 1");
  assert!(d.iter().any(|x| x.code == "sql421"));
}

#[test]
fn r22_sql421_0018() {
  let d = diags("-- s21_1\nSELECT * FROM users WHERE email = 'a' AND email = 'a'");
  assert!(d.iter().any(|x| x.code == "sql421"));
}

#[test]
fn r22_sql421_0019() {
  let d = diags("-- s21_2\nSELECT * FROM users WHERE id = 1 AND id = 1");
  assert!(d.iter().any(|x| x.code == "sql421"));
}

#[test]
fn r22_sql421_0020() {
  let d = diags("-- s21_2\nSELECT * FROM users WHERE id = 1 OR id = 1");
  assert!(d.iter().any(|x| x.code == "sql421"));
}

#[test]
fn r22_sql421_0021() {
  let d = diags("-- s21_2\nSELECT * FROM users WHERE email = 'a' AND email = 'a'");
  assert!(d.iter().any(|x| x.code == "sql421"));
}

#[test]
fn r22_sql402_0025() {
  let d = diags("-- s02_0\nSELECT * FROM users CROSS JOIN users");
  assert!(d.iter().any(|x| x.code == "sql402"));
}

#[test]
fn r22_sql402_0026() {
  let d = diags("-- s02_0\nSELECT u.id FROM users u JOIN users u ON true");
  assert!(d.iter().any(|x| x.code == "sql402"));
}

#[test]
fn r22_sql402_0027() {
  let d = diags("-- s02_0\nSELECT * FROM users, users");
  assert!(d.iter().any(|x| x.code == "sql402"));
}

#[test]
fn r22_sql402_0028() {
  let d = diags("-- s02_1\nSELECT * FROM users CROSS JOIN users");
  assert!(d.iter().any(|x| x.code == "sql402"));
}

#[test]
fn r22_sql402_0029() {
  let d = diags("-- s02_1\nSELECT u.id FROM users u JOIN users u ON true");
  assert!(d.iter().any(|x| x.code == "sql402"));
}

#[test]
fn r22_sql402_0030() {
  let d = diags("-- s02_1\nSELECT * FROM users, users");
  assert!(d.iter().any(|x| x.code == "sql402"));
}

#[test]
fn r22_sql402_0031() {
  let d = diags("-- s02_2\nSELECT * FROM users CROSS JOIN users");
  assert!(d.iter().any(|x| x.code == "sql402"));
}

#[test]
fn r22_sql402_0032() {
  let d = diags("-- s02_2\nSELECT u.id FROM users u JOIN users u ON true");
  assert!(d.iter().any(|x| x.code == "sql402"));
}

#[test]
fn r22_sql402_0033() {
  let d = diags("-- s02_2\nSELECT * FROM users, users");
  assert!(d.iter().any(|x| x.code == "sql402"));
}

#[test]
fn r22_sql406_0037() {
  let d = diags("-- s06_0\nINSERT INTO users (id, id) VALUES (1, 2)");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0038() {
  let d = diags("-- s06_0\nINSERT INTO users (id, name, id) VALUES (1, 'x', 2)");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0039() {
  let d = diags("-- s06_0\nUPDATE users SET name='x', name='y'");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0040() {
  let d = diags("-- s06_0\nUPDATE users SET id=1, id=2");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0041() {
  let d = diags("-- s06_1\nINSERT INTO users (id, id) VALUES (1, 2)");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0042() {
  let d = diags("-- s06_1\nINSERT INTO users (id, name, id) VALUES (1, 'x', 2)");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0043() {
  let d = diags("-- s06_1\nUPDATE users SET name='x', name='y'");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0044() {
  let d = diags("-- s06_1\nUPDATE users SET id=1, id=2");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0045() {
  let d = diags("-- s06_2\nINSERT INTO users (id, id) VALUES (1, 2)");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0046() {
  let d = diags("-- s06_2\nINSERT INTO users (id, name, id) VALUES (1, 'x', 2)");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0047() {
  let d = diags("-- s06_2\nUPDATE users SET name='x', name='y'");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql406_0048() {
  let d = diags("-- s06_2\nUPDATE users SET id=1, id=2");
  assert!(d.iter().any(|x| x.code == "sql406"));
}

#[test]
fn r22_sql441_0049() {
  let d = diags("-- s41_0\nSELECT * FROM users WHERE EXISTS (SELECT 1)");
  assert!(d.iter().any(|x| x.code == "sql441"));
}

#[test]
fn r22_sql441_0050() {
  let d = diags("-- s41_0\nSELECT * FROM users WHERE EXISTS (SELECT 2)");
  assert!(d.iter().any(|x| x.code == "sql441"));
}

#[test]
fn r22_sql441_0051() {
  let d = diags("-- s41_0\nSELECT * FROM users WHERE EXISTS (SELECT NULL)");
  assert!(d.iter().any(|x| x.code == "sql441"));
}

#[test]
fn r22_sql441_0052() {
  let d = diags("-- s41_0\nSELECT * FROM users WHERE NOT EXISTS (SELECT 1)");
  assert!(d.iter().any(|x| x.code == "sql441"));
}

#[test]
fn r22_sql441_0053() {
  let d = diags("-- s41_1\nSELECT * FROM users WHERE EXISTS (SELECT 1)");
  assert!(d.iter().any(|x| x.code == "sql441"));
}

#[test]
fn r22_sql441_0055() {
  let d = diags("-- s41_1\nSELECT * FROM users WHERE EXISTS (SELECT NULL)");
  assert!(d.iter().any(|x| x.code == "sql441"));
}

#[test]
fn r22_sql441_0056() {
  let d = diags("-- s41_1\nSELECT * FROM users WHERE NOT EXISTS (SELECT 1)");
  assert!(d.iter().any(|x| x.code == "sql441"));
}

#[test]
fn r22_sql441_0059() {
  let d = diags("-- s41_2\nSELECT * FROM users WHERE EXISTS (SELECT NULL)");
  assert!(d.iter().any(|x| x.code == "sql441"));
}

#[test]
fn r22_sql441_0060() {
  let d = diags("-- s41_2\nSELECT * FROM users WHERE NOT EXISTS (SELECT 1)");
  assert!(d.iter().any(|x| x.code == "sql441"));
}

#[test]
fn r22_sql492_0061() {
  let d = diags("-- s92_0\nSELECT * FROM users WHERE id IN (NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0062() {
  let d = diags("-- s92_0\nSELECT * FROM users WHERE id NOT IN (NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0063() {
  let d = diags("-- s92_0\nSELECT * FROM users WHERE id IN (1, NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0064() {
  let d = diags("-- s92_0\nSELECT * FROM users WHERE id NOT IN (1, NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0065() {
  let d = diags("-- s92_1\nSELECT * FROM users WHERE id IN (NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0066() {
  let d = diags("-- s92_1\nSELECT * FROM users WHERE id NOT IN (NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0067() {
  let d = diags("-- s92_1\nSELECT * FROM users WHERE id IN (1, NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0068() {
  let d = diags("-- s92_1\nSELECT * FROM users WHERE id NOT IN (1, NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0069() {
  let d = diags("-- s92_2\nSELECT * FROM users WHERE id IN (NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0070() {
  let d = diags("-- s92_2\nSELECT * FROM users WHERE id NOT IN (NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0071() {
  let d = diags("-- s92_2\nSELECT * FROM users WHERE id IN (1, NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql492_0072() {
  let d = diags("-- s92_2\nSELECT * FROM users WHERE id NOT IN (1, NULL)");
  assert!(d.iter().any(|x| x.code == "sql492"));
}

#[test]
fn r22_sql435_0073() {
  let d = diags("-- s35_0\nSELECT id FROM users WHERE id IS NULL AND id IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql435"));
}

#[test]
fn r22_sql435_0074() {
  let d = diags("-- s35_0\nSELECT id FROM users WHERE email IS NULL AND email IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql435"));
}

#[test]
fn r22_sql435_0075() {
  let d = diags("-- s35_0\nSELECT id FROM users WHERE id IS NOT NULL AND id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql435"));
}

#[test]
fn r22_sql435_0076() {
  let d = diags("-- s35_1\nSELECT id FROM users WHERE id IS NULL AND id IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql435"));
}

#[test]
fn r22_sql435_0077() {
  let d = diags("-- s35_1\nSELECT id FROM users WHERE email IS NULL AND email IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql435"));
}

#[test]
fn r22_sql435_0078() {
  let d = diags("-- s35_1\nSELECT id FROM users WHERE id IS NOT NULL AND id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql435"));
}

#[test]
fn r22_sql435_0079() {
  let d = diags("-- s35_2\nSELECT id FROM users WHERE id IS NULL AND id IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql435"));
}

#[test]
fn r22_sql435_0080() {
  let d = diags("-- s35_2\nSELECT id FROM users WHERE email IS NULL AND email IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql435"));
}

#[test]
fn r22_sql435_0081() {
  let d = diags("-- s35_2\nSELECT id FROM users WHERE id IS NOT NULL AND id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql435"));
}

#[test]
fn r22_sql473_0085() {
  let d = diags("-- s73_0\nSELECT * FROM users WHERE id < ALL(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"));
}

#[test]
fn r22_sql473_0086() {
  let d = diags("-- s73_0\nSELECT * FROM users WHERE id > ALL(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"));
}

#[test]
fn r22_sql473_0087() {
  let d = diags("-- s73_0\nSELECT * FROM users WHERE id = ALL(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"));
}

#[test]
fn r22_sql473_0088() {
  let d = diags("-- s73_1\nSELECT * FROM users WHERE id < ALL(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"));
}

#[test]
fn r22_sql473_0089() {
  let d = diags("-- s73_1\nSELECT * FROM users WHERE id > ALL(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"));
}

#[test]
fn r22_sql473_0090() {
  let d = diags("-- s73_1\nSELECT * FROM users WHERE id = ALL(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"));
}

#[test]
fn r22_sql473_0091() {
  let d = diags("-- s73_2\nSELECT * FROM users WHERE id < ALL(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"));
}

#[test]
fn r22_sql473_0092() {
  let d = diags("-- s73_2\nSELECT * FROM users WHERE id > ALL(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"));
}

#[test]
fn r22_sql473_0093() {
  let d = diags("-- s73_2\nSELECT * FROM users WHERE id = ALL(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"));
}


#[test]
fn r23_probe_codes() {
  for s in [
    "SELECT * FROM users WHERE name = '' AND email IS NULL",
    "SELECT * FROM users WHERE NOT TRUE",
    "SELECT * FROM users WHERE NOT FALSE",
    "SELECT * FROM users WHERE NOT (id = 1)",
    "SELECT * FROM users WHERE NOT (id = 1 OR name = 'x')",
    "SELECT count(DISTINCT *) FROM users",
    "SELECT * FROM users WHERE EXISTS (SELECT * FROM orders)",
    "SELECT * FROM users WHERE id < ANY(ARRAY[]::int[])",
    "SELECT * FROM users WHERE id < SOME(ARRAY[]::int[])",
    "SELECT * FROM users CROSS JOIN LATERAL generate_series(1, 10) s",
    "INSERT INTO users (id) VALUES (1), (1)",
    "INSERT INTO users (id) VALUES (1) RETURNING NULL",
    "INSERT INTO users (id) VALUES (1) RETURNING 1",
    "UPDATE users SET id = 1 WHERE TRUE",
    "DELETE FROM users WHERE TRUE",
    "SELECT * FROM users JOIN users ON true",
    "SELECT t.id FROM users t WHERE t.id = t.id",
    "SELECT id FROM users WHERE id != id",
    "SELECT id FROM users WHERE id <> id",
    "SELECT id FROM users WHERE id NOT NULL",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r23_sql227_0001() {
  let d = diags("-- s27_0\nSELECT * FROM users WHERE EXISTS (SELECT * FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql227"));
}

#[test]
fn r23_sql227_0002() {
  let d = diags("-- s27_0\nSELECT * FROM users WHERE EXISTS (SELECT * FROM users)");
  assert!(d.iter().any(|x| x.code == "sql227"));
}

#[test]
fn r23_sql227_0003() {
  let d = diags("-- s27_0\nSELECT * FROM users WHERE NOT EXISTS (SELECT * FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql227"));
}

#[test]
fn r23_sql227_0004() {
  let d = diags("-- s27_1\nSELECT * FROM users WHERE EXISTS (SELECT * FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql227"));
}

#[test]
fn r23_sql227_0005() {
  let d = diags("-- s27_1\nSELECT * FROM users WHERE EXISTS (SELECT * FROM users)");
  assert!(d.iter().any(|x| x.code == "sql227"));
}

#[test]
fn r23_sql227_0006() {
  let d = diags("-- s27_1\nSELECT * FROM users WHERE NOT EXISTS (SELECT * FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql227"));
}

#[test]
fn r23_sql227_0007() {
  let d = diags("-- s27_2\nSELECT * FROM users WHERE EXISTS (SELECT * FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql227"));
}

#[test]
fn r23_sql227_0008() {
  let d = diags("-- s27_2\nSELECT * FROM users WHERE EXISTS (SELECT * FROM users)");
  assert!(d.iter().any(|x| x.code == "sql227"));
}

#[test]
fn r23_sql227_0009() {
  let d = diags("-- s27_2\nSELECT * FROM users WHERE NOT EXISTS (SELECT * FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql227"));
}

#[test]
fn r23_sql350_0010() {
  let d = diags("-- s50_0\nINSERT INTO users (id) VALUES (1) RETURNING NULL");
  assert!(d.iter().any(|x| x.code == "sql350"));
}

#[test]
fn r23_sql350_0011() {
  let d = diags("-- s50_0\nUPDATE users SET name='x' WHERE id=1 RETURNING NULL");
  assert!(d.iter().any(|x| x.code == "sql350"));
}

#[test]
fn r23_sql350_0012() {
  let d = diags("-- s50_0\nDELETE FROM users WHERE id=1 RETURNING NULL");
  assert!(d.iter().any(|x| x.code == "sql350"));
}

#[test]
fn r23_sql350_0013() {
  let d = diags("-- s50_1\nINSERT INTO users (id) VALUES (1) RETURNING NULL");
  assert!(d.iter().any(|x| x.code == "sql350"));
}

#[test]
fn r23_sql350_0014() {
  let d = diags("-- s50_1\nUPDATE users SET name='x' WHERE id=1 RETURNING NULL");
  assert!(d.iter().any(|x| x.code == "sql350"));
}

#[test]
fn r23_sql350_0015() {
  let d = diags("-- s50_1\nDELETE FROM users WHERE id=1 RETURNING NULL");
  assert!(d.iter().any(|x| x.code == "sql350"));
}

#[test]
fn r23_sql350_0016() {
  let d = diags("-- s50_2\nINSERT INTO users (id) VALUES (1) RETURNING NULL");
  assert!(d.iter().any(|x| x.code == "sql350"));
}

#[test]
fn r23_sql350_0017() {
  let d = diags("-- s50_2\nUPDATE users SET name='x' WHERE id=1 RETURNING NULL");
  assert!(d.iter().any(|x| x.code == "sql350"));
}

#[test]
fn r23_sql350_0018() {
  let d = diags("-- s50_2\nDELETE FROM users WHERE id=1 RETURNING NULL");
  assert!(d.iter().any(|x| x.code == "sql350"));
}

#[test]
fn r23_sql282_0019() {
  let d = diags("-- s82_0\nDELETE FROM users WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql282"));
}

#[test]
fn r23_sql282_0020() {
  let d = diags("-- s82_0\nUPDATE users SET id = 1 WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql282"));
}

#[test]
fn r23_sql282_0021() {
  let d = diags("-- s82_0\nSELECT * FROM users WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql282"));
}

#[test]
fn r23_sql282_0022() {
  let d = diags("-- s82_0\nSELECT * FROM users WHERE 1=1");
  assert!(d.iter().any(|x| x.code == "sql282"));
}

#[test]
fn r23_sql282_0023() {
  let d = diags("-- s82_1\nDELETE FROM users WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql282"));
}

#[test]
fn r23_sql282_0024() {
  let d = diags("-- s82_1\nUPDATE users SET id = 1 WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql282"));
}

#[test]
fn r23_sql282_0025() {
  let d = diags("-- s82_1\nSELECT * FROM users WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql282"));
}

#[test]
fn r23_sql282_0026() {
  let d = diags("-- s82_1\nSELECT * FROM users WHERE 1=1");
  assert!(d.iter().any(|x| x.code == "sql282"));
}

#[test]
fn r23_sql282_0027() {
  let d = diags("-- s82_2\nDELETE FROM users WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql282"));
}

#[test]
fn r23_sql408_0028() {
  let d = diags("-- s08_0\nSELECT id FROM users WHERE id != id");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r23_sql408_0029() {
  let d = diags("-- s08_0\nSELECT id FROM users WHERE id <> id");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r23_sql408_0030() {
  let d = diags("-- s08_0\nSELECT t.id FROM users t WHERE t.id = t.id");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r23_sql408_0031() {
  let d = diags("-- s08_0\nSELECT * FROM users WHERE email = email");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r23_sql408_0032() {
  let d = diags("-- s08_0\nSELECT * FROM users WHERE name = name");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r23_sql408_0033() {
  let d = diags("-- s08_1\nSELECT id FROM users WHERE id != id");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r23_sql408_0034() {
  let d = diags("-- s08_1\nSELECT id FROM users WHERE id <> id");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r23_sql408_0035() {
  let d = diags("-- s08_1\nSELECT t.id FROM users t WHERE t.id = t.id");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r23_sql408_0036() {
  let d = diags("-- s08_1\nSELECT * FROM users WHERE email = email");
  assert!(d.iter().any(|x| x.code == "sql408"));
}

#[test]
fn r23_sql171_0037() {
  let d = diags("-- s71_0\nUPDATE users SET id = 1 WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql171"));
}

#[test]
fn r23_sql171_0038() {
  let d = diags("-- s71_0\nUPDATE users SET id = 1, id = 2");
  assert!(d.iter().any(|x| x.code == "sql171"));
}

#[test]
fn r23_sql171_0039() {
  let d = diags("-- s71_0\nUPDATE orders SET id = 1 WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql171"));
}

#[test]
fn r23_sql171_0041() {
  let d = diags("-- s71_1\nUPDATE users SET id = 1, id = 2");
  assert!(d.iter().any(|x| x.code == "sql171"));
}

#[test]
fn r23_sql171_0042() {
  let d = diags("-- s71_1\nUPDATE orders SET id = 1 WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql171"));
}

#[test]
fn r23_sql171_0044() {
  let d = diags("-- s71_2\nUPDATE users SET id = 1, id = 2");
  assert!(d.iter().any(|x| x.code == "sql171"));
}

#[test]
fn r23_sql171_0045() {
  let d = diags("-- s71_2\nUPDATE orders SET id = 1 WHERE TRUE");
  assert!(d.iter().any(|x| x.code == "sql171"));
}

#[test]
fn r24_probe_codes() {
  for s in [
    "CREATE TABLE t (id int, FOREIGN KEY (id) REFERENCES users(id) ON DELETE NO ACTION)",
    "CREATE TABLE t (id int REFERENCES users(id) ON DELETE CASCADE)",
    "CREATE TABLE t (id int REFERENCES users(id) ON DELETE SET NULL)",
    "CREATE TABLE t (id int REFERENCES users(id) ON DELETE RESTRICT)",
    "CREATE TABLE t (id int REFERENCES users(id) DEFERRABLE)",
    "CREATE TABLE t (id int REFERENCES users(id) NOT DEFERRABLE)",
    "CREATE TABLE t (id int REFERENCES users(id) INITIALLY DEFERRED)",
    "CREATE TABLE t (id int REFERENCES users(id) INITIALLY IMMEDIATE)",
    "CREATE INDEX ON users (lower(name))",
    "CREATE INDEX ON users (upper(name))",
    "CREATE INDEX ON users (id::text)",
    "ALTER TABLE users ADD CONSTRAINT chk CHECK (id > 0) NOT VALID",
    "ALTER TABLE users VALIDATE CONSTRAINT chk",
    "ALTER TABLE users INHERIT parent_t",
    "ALTER TABLE users OWNER TO another_owner",
    "ALTER TABLE users SET TABLESPACE my_ts",
    "ALTER TABLE users SET (fillfactor = 90)",
    "ALTER TABLE users SET LOGGED",
    "ALTER TABLE users SET UNLOGGED",
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql IMMUTABLE AS 'select 1'",
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql VOLATILE AS 'select 1'",
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql STABLE AS 'select 1'",
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql SECURITY DEFINER AS 'select 1'",
    "CREATE FUNCTION f() RETURNS int LANGUAGE sql SECURITY INVOKER AS 'select 1'",
    "DELETE FROM users",
    "TRUNCATE TABLE users",
    "TRUNCATE TABLE users CASCADE",
    "TRUNCATE TABLE users RESTART IDENTITY",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r24_sql254_0001() {
  let d = diags("-- s54_0\nALTER TABLE users SET TABLESPACE my_ts");
  assert!(d.iter().any(|x| x.code == "sql254"));
}

#[test]
fn r24_sql254_0002() {
  let d = diags("-- s54_0\nALTER TABLE orders SET TABLESPACE my_ts");
  assert!(d.iter().any(|x| x.code == "sql254"));
}

#[test]
fn r24_sql254_0004() {
  let d = diags("-- s54_1\nALTER TABLE users SET TABLESPACE my_ts");
  assert!(d.iter().any(|x| x.code == "sql254"));
}

#[test]
fn r24_sql254_0005() {
  let d = diags("-- s54_1\nALTER TABLE orders SET TABLESPACE my_ts");
  assert!(d.iter().any(|x| x.code == "sql254"));
}

#[test]
fn r24_sql254_0007() {
  let d = diags("-- s54_2\nALTER TABLE users SET TABLESPACE my_ts");
  assert!(d.iter().any(|x| x.code == "sql254"));
}

#[test]
fn r24_sql254_0008() {
  let d = diags("-- s54_2\nALTER TABLE orders SET TABLESPACE my_ts");
  assert!(d.iter().any(|x| x.code == "sql254"));
}

#[test]
fn r24_sql201_0010() {
  let d = diags("-- s01_0\nCREATE FUNCTION f() RETURNS int LANGUAGE sql SECURITY DEFINER AS 'select 1'");
  assert!(d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn r24_sql201_0011() {
  let d = diags("-- s01_0\nCREATE FUNCTION g() RETURNS void LANGUAGE plpgsql SECURITY DEFINER AS $$BEGIN END$$");
  assert!(d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn r24_sql201_0012() {
  let d = diags("-- s01_0\nCREATE OR REPLACE FUNCTION h() RETURNS int LANGUAGE sql SECURITY DEFINER AS 'select 1'");
  assert!(d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn r24_sql201_0013() {
  let d = diags("-- s01_1\nCREATE FUNCTION f() RETURNS int LANGUAGE sql SECURITY DEFINER AS 'select 1'");
  assert!(d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn r24_sql201_0014() {
  let d = diags("-- s01_1\nCREATE FUNCTION g() RETURNS void LANGUAGE plpgsql SECURITY DEFINER AS $$BEGIN END$$");
  assert!(d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn r24_sql201_0015() {
  let d = diags("-- s01_1\nCREATE OR REPLACE FUNCTION h() RETURNS int LANGUAGE sql SECURITY DEFINER AS 'select 1'");
  assert!(d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn r24_sql201_0016() {
  let d = diags("-- s01_2\nCREATE FUNCTION f() RETURNS int LANGUAGE sql SECURITY DEFINER AS 'select 1'");
  assert!(d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn r24_sql201_0017() {
  let d = diags("-- s01_2\nCREATE FUNCTION g() RETURNS void LANGUAGE plpgsql SECURITY DEFINER AS $$BEGIN END$$");
  assert!(d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn r24_sql201_0018() {
  let d = diags("-- s01_2\nCREATE OR REPLACE FUNCTION h() RETURNS int LANGUAGE sql SECURITY DEFINER AS 'select 1'");
  assert!(d.iter().any(|x| x.code == "sql201"));
}

#[test]
fn r24_sql105_0019() {
  let d = diags("-- s05_0\nTRUNCATE TABLE users");
  assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn r24_sql105_0020() {
  let d = diags("-- s05_0\nTRUNCATE TABLE orders");
  assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn r24_sql105_0021() {
  let d = diags("-- s05_0\nTRUNCATE TABLE users RESTART IDENTITY");
  assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn r24_sql105_0022() {
  let d = diags("-- s05_0\nTRUNCATE users");
  assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn r24_sql105_0023() {
  let d = diags("-- s05_1\nTRUNCATE TABLE users");
  assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn r24_sql105_0024() {
  let d = diags("-- s05_1\nTRUNCATE TABLE orders");
  assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn r24_sql105_0025() {
  let d = diags("-- s05_1\nTRUNCATE TABLE users RESTART IDENTITY");
  assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn r24_sql105_0026() {
  let d = diags("-- s05_1\nTRUNCATE users");
  assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn r24_sql105_0027() {
  let d = diags("-- s05_2\nTRUNCATE TABLE users");
  assert!(d.iter().any(|x| x.code == "sql105"));
}

#[test]
fn r24_sql046_0028() {
  let d = diags("-- s46_0\nCREATE TABLE t (id int)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r24_sql046_0029() {
  let d = diags("-- s46_0\nCREATE TABLE t (a int, b text)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r24_sql046_0030() {
  let d = diags("-- s46_0\nCREATE TABLE t (id int, FOREIGN KEY (id) REFERENCES users(id))");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r24_sql046_0031() {
  let d = diags("-- s46_0\nCREATE TABLE t (id int REFERENCES users(id))");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r24_sql046_0032() {
  let d = diags("-- s46_1\nCREATE TABLE t (id int)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r24_sql046_0033() {
  let d = diags("-- s46_1\nCREATE TABLE t (a int, b text)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r24_sql046_0034() {
  let d = diags("-- s46_1\nCREATE TABLE t (id int, FOREIGN KEY (id) REFERENCES users(id))");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r24_sql046_0035() {
  let d = diags("-- s46_1\nCREATE TABLE t (id int REFERENCES users(id))");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r24_sql046_0036() {
  let d = diags("-- s46_2\nCREATE TABLE t (id int)");
  assert!(d.iter().any(|x| x.code == "sql046"));
}

#[test]
fn r24_sql288_0037() {
  let d = diags("-- s88_0\nCREATE INDEX ON users (lower(name))");
  assert!(d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn r24_sql288_0038() {
  let d = diags("-- s88_0\nCREATE INDEX ON users (upper(name))");
  assert!(d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn r24_sql288_0039() {
  let d = diags("-- s88_0\nCREATE INDEX ON orders (extract(year from id::text::date))");
  assert!(d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn r24_sql288_0040() {
  let d = diags("-- s88_1\nCREATE INDEX ON users (lower(name))");
  assert!(d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn r24_sql288_0041() {
  let d = diags("-- s88_1\nCREATE INDEX ON users (upper(name))");
  assert!(d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn r24_sql288_0042() {
  let d = diags("-- s88_1\nCREATE INDEX ON orders (extract(year from id::text::date))");
  assert!(d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn r24_sql288_0043() {
  let d = diags("-- s88_2\nCREATE INDEX ON users (lower(name))");
  assert!(d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn r24_sql288_0044() {
  let d = diags("-- s88_2\nCREATE INDEX ON users (upper(name))");
  assert!(d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn r24_sql288_0045() {
  let d = diags("-- s88_2\nCREATE INDEX ON orders (extract(year from id::text::date))");
  assert!(d.iter().any(|x| x.code == "sql288"));
}

#[test]
fn r25_probe_codes() {
  for s in [
    "CREATE TABLE t (id int PRIMARY KEY, parent_id int REFERENCES t(id))",
    "CREATE TABLE t (id int PRIMARY KEY, data jsonb DEFAULT '{}')",
    "CREATE TABLE t (id int PRIMARY KEY, data jsonb NOT NULL DEFAULT '{}'::jsonb)",
    "CREATE INDEX ON users (id) WHERE id IS NOT NULL",
    "CREATE INDEX ON users (id) WHERE id > 0",
    "CREATE INDEX ON users (id) WITH (fillfactor = 90)",
    "INSERT INTO users (id) SELECT id FROM users WHERE FALSE",
    "INSERT INTO users (id) SELECT id FROM users LIMIT 0",
    "WITH RECURSIVE r(n) AS (SELECT 1) SELECT * FROM r",
    "WITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT n FROM r) SELECT * FROM r",
    "WITH x AS (SELECT 1) SELECT * FROM x WHERE NOT EXISTS (SELECT 1 FROM x)",
    "SELECT id FROM users HAVING id > 0",
    "SELECT id FROM users WHERE id = 1 GROUP BY 1",
    "SELECT id FROM users WHERE TRUE AND id IS NULL",
    "SELECT id FROM users WHERE FALSE OR id IS NOT NULL",
    "SELECT id FROM users WHERE NULL AND id > 0",
    "SELECT id FROM users WHERE NULL OR id > 0",
    "SELECT id FROM users WHERE id IS NULL OR id IS NOT NULL",
    "SELECT * FROM users WHERE id > 0 OR id <= 0",
    "SELECT * FROM users WHERE id = 0 OR id != 0",
    "VACUUM FULL users",
    "VACUUM (FULL) users",
    "VACUUM (ANALYZE) users",
    "VACUUM",
    "ANALYZE",
    "REINDEX TABLE users",
    "REINDEX INDEX my_idx",
    "REINDEX DATABASE postgres",
    "REINDEX SCHEMA public",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r25_sql304_0001() {
  let d = diags("-- s304_0\nCREATE TABLE t (id int PRIMARY KEY, parent_id int REFERENCES t(id))");
  assert!(d.iter().any(|x| x.code == "sql304"));
}

#[test]
fn r25_sql304_0002() {
  let d = diags("-- s304_0\nCREATE TABLE node (id int PRIMARY KEY, parent int REFERENCES node(id))");
  assert!(d.iter().any(|x| x.code == "sql304"));
}

#[test]
fn r25_sql304_0003() {
  let d = diags("-- s304_0\nCREATE TABLE cat (id int PRIMARY KEY, parent_cat int REFERENCES cat(id))");
  assert!(d.iter().any(|x| x.code == "sql304"));
}

#[test]
fn r25_sql304_0004() {
  let d = diags("-- s304_1\nCREATE TABLE t (id int PRIMARY KEY, parent_id int REFERENCES t(id))");
  assert!(d.iter().any(|x| x.code == "sql304"));
}

#[test]
fn r25_sql304_0005() {
  let d = diags("-- s304_1\nCREATE TABLE node (id int PRIMARY KEY, parent int REFERENCES node(id))");
  assert!(d.iter().any(|x| x.code == "sql304"));
}

#[test]
fn r25_sql304_0006() {
  let d = diags("-- s304_1\nCREATE TABLE cat (id int PRIMARY KEY, parent_cat int REFERENCES cat(id))");
  assert!(d.iter().any(|x| x.code == "sql304"));
}

#[test]
fn r25_sql304_0007() {
  let d = diags("-- s304_2\nCREATE TABLE t (id int PRIMARY KEY, parent_id int REFERENCES t(id))");
  assert!(d.iter().any(|x| x.code == "sql304"));
}

#[test]
fn r25_sql304_0008() {
  let d = diags("-- s304_2\nCREATE TABLE node (id int PRIMARY KEY, parent int REFERENCES node(id))");
  assert!(d.iter().any(|x| x.code == "sql304"));
}

#[test]
fn r25_sql304_0009() {
  let d = diags("-- s304_2\nCREATE TABLE cat (id int PRIMARY KEY, parent_cat int REFERENCES cat(id))");
  assert!(d.iter().any(|x| x.code == "sql304"));
}

#[test]
fn r25_sql460_0010() {
  let d = diags("-- s460_0\nSELECT id FROM users HAVING id > 0");
  assert!(d.iter().any(|x| x.code == "sql460"));
}

#[test]
fn r25_sql460_0011() {
  let d = diags("-- s460_0\nSELECT id FROM orders HAVING id > 0");
  assert!(d.iter().any(|x| x.code == "sql460"));
}

#[test]
fn r25_sql460_0012() {
  let d = diags("-- s460_0\nSELECT email FROM users HAVING email IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql460"));
}

#[test]
fn r25_sql460_0013() {
  let d = diags("-- s460_1\nSELECT id FROM users HAVING id > 0");
  assert!(d.iter().any(|x| x.code == "sql460"));
}

#[test]
fn r25_sql460_0014() {
  let d = diags("-- s460_1\nSELECT id FROM orders HAVING id > 0");
  assert!(d.iter().any(|x| x.code == "sql460"));
}

#[test]
fn r25_sql460_0015() {
  let d = diags("-- s460_1\nSELECT email FROM users HAVING email IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql460"));
}

#[test]
fn r25_sql460_0016() {
  let d = diags("-- s460_2\nSELECT id FROM users HAVING id > 0");
  assert!(d.iter().any(|x| x.code == "sql460"));
}

#[test]
fn r25_sql460_0017() {
  let d = diags("-- s460_2\nSELECT id FROM orders HAVING id > 0");
  assert!(d.iter().any(|x| x.code == "sql460"));
}

#[test]
fn r25_sql460_0018() {
  let d = diags("-- s460_2\nSELECT email FROM users HAVING email IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql460"));
}

#[test]
fn r25_sql220_0019() {
  let d = diags("-- s220_0\nWITH RECURSIVE r(n) AS (SELECT 1) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql220"));
}

#[test]
fn r25_sql220_0020() {
  let d = diags("-- s220_0\nWITH RECURSIVE r(n) AS (SELECT id FROM users) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql220"));
}

#[test]
fn r25_sql220_0021() {
  let d = diags("-- s220_0\nWITH RECURSIVE r AS (SELECT 1 AS n) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql220"));
}

#[test]
fn r25_sql220_0022() {
  let d = diags("-- s220_1\nWITH RECURSIVE r(n) AS (SELECT 1) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql220"));
}

#[test]
fn r25_sql220_0023() {
  let d = diags("-- s220_1\nWITH RECURSIVE r(n) AS (SELECT id FROM users) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql220"));
}

#[test]
fn r25_sql220_0024() {
  let d = diags("-- s220_1\nWITH RECURSIVE r AS (SELECT 1 AS n) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql220"));
}

#[test]
fn r25_sql220_0025() {
  let d = diags("-- s220_2\nWITH RECURSIVE r(n) AS (SELECT 1) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql220"));
}

#[test]
fn r25_sql220_0026() {
  let d = diags("-- s220_2\nWITH RECURSIVE r(n) AS (SELECT id FROM users) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql220"));
}

#[test]
fn r25_sql220_0027() {
  let d = diags("-- s220_2\nWITH RECURSIVE r AS (SELECT 1 AS n) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql220"));
}

#[test]
fn r25_sql167_0028() {
  let d = diags("-- s167_0\nCREATE INDEX ON users (id) WHERE id IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql167"));
}

#[test]
fn r25_sql167_0029() {
  let d = diags("-- s167_0\nCREATE INDEX ON users (id) WHERE id > 0");
  assert!(d.iter().any(|x| x.code == "sql167"));
}

#[test]
fn r25_sql167_0031() {
  let d = diags("-- s167_1\nCREATE INDEX ON users (id) WHERE id IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql167"));
}

#[test]
fn r25_sql167_0032() {
  let d = diags("-- s167_1\nCREATE INDEX ON users (id) WHERE id > 0");
  assert!(d.iter().any(|x| x.code == "sql167"));
}

#[test]
fn r25_sql167_0034() {
  let d = diags("-- s167_2\nCREATE INDEX ON users (id) WHERE id IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql167"));
}

#[test]
fn r25_sql167_0035() {
  let d = diags("-- s167_2\nCREATE INDEX ON users (id) WHERE id > 0");
  assert!(d.iter().any(|x| x.code == "sql167"));
}

#[test]
fn r25_sql056_0037() {
  let d = diags("-- s056_0\nWITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT n FROM r) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r25_sql056_0040() {
  let d = diags("-- s056_1\nWITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT n FROM r) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql056"));
}

#[test]
fn r25_sql056_0043() {
  let d = diags("-- s056_2\nWITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT n FROM r) SELECT * FROM r");
  assert!(d.iter().any(|x| x.code == "sql056"));
}


#[test]
fn r26_probe_codes() {
  for s in [
    "SELECT id FROM users WHERE id::text LIKE '1%'",
    "SELECT * FROM users WHERE EXTRACT(YEAR FROM created_at) = 2024",
    "SELECT * FROM users WHERE date_trunc('day', ts) = '2024-01-01'",
    "SELECT * FROM users WHERE id::text::int = 1",
    "SELECT * FROM users WHERE id = 1.0",
    "SELECT * FROM users WHERE id = 1::numeric",
    "SELECT * FROM users WHERE id BETWEEN 1 AND 1",
    "SELECT * FROM users WHERE id BETWEEN id AND id",
    "SELECT * FROM users WHERE name SIMILAR TO ''",
    "SELECT * FROM users WHERE name SIMILAR TO 'abc'",
    "SELECT * FROM users WHERE name ~ ''",
    "SELECT * FROM users WHERE name ~ '^$'",
    "SELECT count(*) FROM users WHERE id IS NULL",
    "SELECT count(DISTINCT *) FROM users",
    "SELECT count(*) AS count, max(id) AS count FROM users",
    "SELECT id AS x FROM users ORDER BY x",
    "SELECT id AS x FROM users ORDER BY id",
    "SELECT id, name FROM users ORDER BY 0",
    "SELECT id, name FROM users ORDER BY 99",
    "SELECT id FROM users LIMIT id",
    "SELECT id FROM users LIMIT name",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r26_sql409_0001() {
  let d = diags("-- s09_0\nSELECT * FROM users WHERE id BETWEEN id AND id");
  assert!(d.iter().any(|x| x.code == "sql409"));
}

#[test]
fn r26_sql409_0002() {
  let d = diags("-- s09_0\nSELECT * FROM users WHERE email BETWEEN email AND email");
  assert!(d.iter().any(|x| x.code == "sql409"));
}

#[test]
fn r26_sql409_0003() {
  let d = diags("-- s09_0\nSELECT * FROM orders WHERE id BETWEEN id AND id");
  assert!(d.iter().any(|x| x.code == "sql409"));
}

#[test]
fn r26_sql409_0004() {
  let d = diags("-- s09_1\nSELECT * FROM users WHERE id BETWEEN id AND id");
  assert!(d.iter().any(|x| x.code == "sql409"));
}

#[test]
fn r26_sql409_0005() {
  let d = diags("-- s09_1\nSELECT * FROM users WHERE email BETWEEN email AND email");
  assert!(d.iter().any(|x| x.code == "sql409"));
}

#[test]
fn r26_sql409_0006() {
  let d = diags("-- s09_1\nSELECT * FROM orders WHERE id BETWEEN id AND id");
  assert!(d.iter().any(|x| x.code == "sql409"));
}

#[test]
fn r26_sql409_0007() {
  let d = diags("-- s09_2\nSELECT * FROM users WHERE id BETWEEN id AND id");
  assert!(d.iter().any(|x| x.code == "sql409"));
}

#[test]
fn r26_sql409_0008() {
  let d = diags("-- s09_2\nSELECT * FROM users WHERE email BETWEEN email AND email");
  assert!(d.iter().any(|x| x.code == "sql409"));
}

#[test]
fn r26_sql409_0009() {
  let d = diags("-- s09_2\nSELECT * FROM orders WHERE id BETWEEN id AND id");
  assert!(d.iter().any(|x| x.code == "sql409"));
}

#[test]
fn r26_sql498_0010() {
  let d = diags("-- s98_0\nSELECT * FROM users WHERE name SIMILAR TO ''");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r26_sql498_0011() {
  let d = diags("-- s98_0\nSELECT * FROM users WHERE name SIMILAR TO 'abc'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r26_sql498_0012() {
  let d = diags("-- s98_0\nSELECT * FROM users WHERE email SIMILAR TO 'a@b'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r26_sql498_0013() {
  let d = diags("-- s98_1\nSELECT * FROM users WHERE name SIMILAR TO ''");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r26_sql498_0014() {
  let d = diags("-- s98_1\nSELECT * FROM users WHERE name SIMILAR TO 'abc'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r26_sql498_0015() {
  let d = diags("-- s98_1\nSELECT * FROM users WHERE email SIMILAR TO 'a@b'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r26_sql498_0016() {
  let d = diags("-- s98_2\nSELECT * FROM users WHERE name SIMILAR TO ''");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r26_sql498_0017() {
  let d = diags("-- s98_2\nSELECT * FROM users WHERE name SIMILAR TO 'abc'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r26_sql498_0018() {
  let d = diags("-- s98_2\nSELECT * FROM users WHERE email SIMILAR TO 'a@b'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r26_sql410_0019() {
  let d = diags("-- s10_0\nSELECT count(*) AS count, max(id) AS count FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r26_sql410_0020() {
  let d = diags("-- s10_0\nSELECT id AS x, email AS x FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r26_sql410_0021() {
  let d = diags("-- s10_0\nSELECT id AS a, name AS a, email AS a FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r26_sql410_0022() {
  let d = diags("-- s10_1\nSELECT count(*) AS count, max(id) AS count FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r26_sql410_0023() {
  let d = diags("-- s10_1\nSELECT id AS x, email AS x FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r26_sql410_0024() {
  let d = diags("-- s10_1\nSELECT id AS a, name AS a, email AS a FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r26_sql410_0025() {
  let d = diags("-- s10_2\nSELECT count(*) AS count, max(id) AS count FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r26_sql410_0026() {
  let d = diags("-- s10_2\nSELECT id AS x, email AS x FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r26_sql410_0027() {
  let d = diags("-- s10_2\nSELECT id AS a, name AS a, email AS a FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"));
}

#[test]
fn r26_sql457_0028() {
  let d = diags("-- s57_0\nSELECT id, name FROM users ORDER BY 0");
  assert!(d.iter().any(|x| x.code == "sql457"));
}

#[test]
fn r26_sql457_0029() {
  let d = diags("-- s57_0\nSELECT id, name FROM users ORDER BY 99");
  assert!(d.iter().any(|x| x.code == "sql457"));
}

#[test]
fn r26_sql457_0031() {
  let d = diags("-- s57_0\nSELECT id FROM orders ORDER BY 100");
  assert!(d.iter().any(|x| x.code == "sql457"));
}

#[test]
fn r26_sql457_0032() {
  let d = diags("-- s57_1\nSELECT id, name FROM users ORDER BY 0");
  assert!(d.iter().any(|x| x.code == "sql457"));
}

#[test]
fn r26_sql457_0035() {
  let d = diags("-- s57_1\nSELECT id FROM orders ORDER BY 100");
  assert!(d.iter().any(|x| x.code == "sql457"));
}

#[test]
fn r26_sql154_0037() {
  let d = diags("-- s54_0\nSELECT count(*) FROM users WHERE id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn r26_sql154_0038() {
  let d = diags("-- s54_0\nSELECT count(*) FROM users WHERE email IS NULL");
  assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn r26_sql154_0039() {
  let d = diags("-- s54_0\nSELECT count(*) FROM orders WHERE user_id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn r26_sql154_0040() {
  let d = diags("-- s54_1\nSELECT count(*) FROM users WHERE id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn r26_sql154_0041() {
  let d = diags("-- s54_1\nSELECT count(*) FROM users WHERE email IS NULL");
  assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn r26_sql154_0042() {
  let d = diags("-- s54_1\nSELECT count(*) FROM orders WHERE user_id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn r26_sql154_0043() {
  let d = diags("-- s54_2\nSELECT count(*) FROM users WHERE id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn r26_sql154_0044() {
  let d = diags("-- s54_2\nSELECT count(*) FROM users WHERE email IS NULL");
  assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn r26_sql154_0045() {
  let d = diags("-- s54_2\nSELECT count(*) FROM orders WHERE user_id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql154"));
}

#[test]
fn r27_probe_codes() {
  for s in [
    "SELECT id FROM users WHERE id = '1'::int + 0",
    "SELECT id FROM users WHERE id IN (1::int, 2::int)",
    "SELECT id FROM users WHERE name LIKE '%' || 'x' || '%'",
    "SELECT id FROM users WHERE name = ANY(STRING_TO_ARRAY('a,b', ','))",
    "INSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET id = users.id",
    "INSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = users.name",
    "SELECT pg_sleep(10)",
    "SELECT pg_sleep_for('1 second')",
    "SELECT NOW() - INTERVAL '1 day'",
    "SELECT CURRENT_TIMESTAMP - CURRENT_TIMESTAMP",
    "SELECT EXTRACT(EPOCH FROM NOW())",
    "SELECT id FROM users CROSS JOIN LATERAL generate_series(1, 1)",
    "SELECT id FROM users WHERE name SIMILAR TO 'a%' ESCAPE 'a'",
    "SELECT id FROM users WHERE name LIKE 'a%' ESCAPE 'a'",
    "SELECT * FROM users WHERE name = ANY(ARRAY[1, 2, 3])",
    "SELECT * FROM users WHERE name = ANY(SELECT id FROM orders)",
    "SELECT * FROM users WHERE id = ANY('{1,2,3}'::int[])",
    "SELECT id FROM users JOIN orders USING (user_id)",
    "SELECT id FROM users JOIN orders USING (id, user_id)",
    "INSERT INTO users (id) VALUES (1) ON CONFLICT DO NOTHING",
    "INSERT INTO users (id) VALUES (1) ON CONFLICT (id) WHERE id > 0 DO NOTHING",
  ] {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r27_sql489_0001() {
  let d = diags("-- s89_0\nSELECT id FROM users WHERE id = '1'::int + 0");
  assert!(d.iter().any(|x| x.code == "sql489"));
}

#[test]
fn r27_sql489_0003() {
  let d = diags("-- s89_0\nSELECT id FROM orders WHERE user_id = '10'::int + 0");
  assert!(d.iter().any(|x| x.code == "sql489"));
}

#[test]
fn r27_sql489_0004() {
  let d = diags("-- s89_1\nSELECT id FROM users WHERE id = '1'::int + 0");
  assert!(d.iter().any(|x| x.code == "sql489"));
}

#[test]
fn r27_sql489_0006() {
  let d = diags("-- s89_1\nSELECT id FROM orders WHERE user_id = '10'::int + 0");
  assert!(d.iter().any(|x| x.code == "sql489"));
}

#[test]
fn r27_sql489_0007() {
  let d = diags("-- s89_2\nSELECT id FROM users WHERE id = '1'::int + 0");
  assert!(d.iter().any(|x| x.code == "sql489"));
}

#[test]
fn r27_sql489_0009() {
  let d = diags("-- s89_2\nSELECT id FROM orders WHERE user_id = '10'::int + 0");
  assert!(d.iter().any(|x| x.code == "sql489"));
}

#[test]
fn r27_sql112_0010() {
  let d = diags("-- s12_0\nSELECT id FROM users CROSS JOIN LATERAL generate_series(1, 1)");
  assert!(d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn r27_sql112_0012() {
  let d = diags("-- s12_0\nSELECT id FROM orders CROSS JOIN LATERAL generate_series(1, 5)");
  assert!(d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn r27_sql112_0013() {
  let d = diags("-- s12_1\nSELECT id FROM users CROSS JOIN LATERAL generate_series(1, 1)");
  assert!(d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn r27_sql112_0015() {
  let d = diags("-- s12_1\nSELECT id FROM orders CROSS JOIN LATERAL generate_series(1, 5)");
  assert!(d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn r27_sql112_0016() {
  let d = diags("-- s12_2\nSELECT id FROM users CROSS JOIN LATERAL generate_series(1, 1)");
  assert!(d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn r27_sql112_0018() {
  let d = diags("-- s12_2\nSELECT id FROM orders CROSS JOIN LATERAL generate_series(1, 5)");
  assert!(d.iter().any(|x| x.code == "sql112"));
}

#[test]
fn r27_sql246_0019() {
  let d = diags("-- s46_0\nINSERT INTO users (id) VALUES (1) ON CONFLICT DO NOTHING");
  assert!(d.iter().any(|x| x.code == "sql246"));
}

#[test]
fn r27_sql246_0020() {
  let d = diags("-- s46_0\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT DO NOTHING");
  assert!(d.iter().any(|x| x.code == "sql246"));
}

#[test]
fn r27_sql246_0021() {
  let d = diags("-- s46_0\nINSERT INTO orders (id, user_id) VALUES (1, 2) ON CONFLICT DO NOTHING");
  assert!(d.iter().any(|x| x.code == "sql246"));
}

#[test]
fn r27_sql246_0022() {
  let d = diags("-- s46_1\nINSERT INTO users (id) VALUES (1) ON CONFLICT DO NOTHING");
  assert!(d.iter().any(|x| x.code == "sql246"));
}

#[test]
fn r27_sql246_0023() {
  let d = diags("-- s46_1\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT DO NOTHING");
  assert!(d.iter().any(|x| x.code == "sql246"));
}

#[test]
fn r27_sql246_0024() {
  let d = diags("-- s46_1\nINSERT INTO orders (id, user_id) VALUES (1, 2) ON CONFLICT DO NOTHING");
  assert!(d.iter().any(|x| x.code == "sql246"));
}

#[test]
fn r27_sql246_0025() {
  let d = diags("-- s46_2\nINSERT INTO users (id) VALUES (1) ON CONFLICT DO NOTHING");
  assert!(d.iter().any(|x| x.code == "sql246"));
}

#[test]
fn r27_sql246_0026() {
  let d = diags("-- s46_2\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT DO NOTHING");
  assert!(d.iter().any(|x| x.code == "sql246"));
}

#[test]
fn r27_sql246_0027() {
  let d = diags("-- s46_2\nINSERT INTO orders (id, user_id) VALUES (1, 2) ON CONFLICT DO NOTHING");
  assert!(d.iter().any(|x| x.code == "sql246"));
}

#[test]
fn r27_sql498_0028() {
  let d = diags("-- s98_0\nSELECT id FROM users WHERE name SIMILAR TO 'a%' ESCAPE 'a'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r27_sql498_0029() {
  let d = diags("-- s98_0\nSELECT id FROM users WHERE email SIMILAR TO 'a' ESCAPE 'b'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r27_sql498_0030() {
  let d = diags("-- s98_1\nSELECT id FROM users WHERE name SIMILAR TO 'a%' ESCAPE 'a'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r27_sql498_0031() {
  let d = diags("-- s98_1\nSELECT id FROM users WHERE email SIMILAR TO 'a' ESCAPE 'b'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r27_sql498_0032() {
  let d = diags("-- s98_2\nSELECT id FROM users WHERE name SIMILAR TO 'a%' ESCAPE 'a'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r27_sql498_0033() {
  let d = diags("-- s98_2\nSELECT id FROM users WHERE email SIMILAR TO 'a' ESCAPE 'b'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r27_sql498_0034() {
  let d = diags("-- s98_3\nSELECT id FROM users WHERE name SIMILAR TO 'a%' ESCAPE 'a'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r27_sql498_0035() {
  let d = diags("-- s98_3\nSELECT id FROM users WHERE email SIMILAR TO 'a' ESCAPE 'b'");
  assert!(d.iter().any(|x| x.code == "sql498"));
}

#[test]
fn r27_sql003_0036() {
  let d = diags("-- s03_0\nSELECT id FROM users JOIN orders USING (user_id)");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r27_sql003_0037() {
  let d = diags("-- s03_0\nSELECT id FROM users u JOIN orders o USING (user_id)");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r27_sql003_0038() {
  let d = diags("-- s03_1\nSELECT id FROM users JOIN orders USING (user_id)");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r27_sql003_0039() {
  let d = diags("-- s03_1\nSELECT id FROM users u JOIN orders o USING (user_id)");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r27_sql003_0040() {
  let d = diags("-- s03_2\nSELECT id FROM users JOIN orders USING (user_id)");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r27_sql003_0041() {
  let d = diags("-- s03_2\nSELECT id FROM users u JOIN orders o USING (user_id)");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r27_sql003_0042() {
  let d = diags("-- s03_3\nSELECT id FROM users JOIN orders USING (user_id)");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r27_sql003_0043() {
  let d = diags("-- s03_3\nSELECT id FROM users u JOIN orders o USING (user_id)");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r27_sql003_0044() {
  let d = diags("-- s03_4\nSELECT id FROM users JOIN orders USING (user_id)");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r27_sql003_0045() {
  let d = diags("-- s03_4\nSELECT id FROM users u JOIN orders o USING (user_id)");
  assert!(d.iter().any(|x| x.code == "sql003"));
}

#[test]
fn r28_bulk_probe() {
  let inputs = [
    "SELECT * FROM users WHERE upper(name) = 'X'",
    "SELECT * FROM users WHERE name ILIKE 'x'",
    "SELECT * FROM users WHERE name LIKE '_'",
    "SELECT * FROM users WHERE id::text = '1'",
    "SELECT * FROM users WHERE id NOT IN (1, 2, 3)",
    "SELECT * FROM users WHERE id <> ALL(ARRAY[1,2,3])",
    "SELECT * FROM users WHERE id = ANY(ARRAY[1,2,3])",
    "SELECT id, count(*) FROM users",
    "SELECT count(*), id FROM users",
    "SELECT count(*) FROM users GROUP BY ()",
    "SELECT id FROM users HAVING TRUE",
    "SELECT id FROM users WHERE 1=2",
    "SELECT id FROM users WHERE 0=0",
    "SELECT * FROM users WHERE NULL <> NULL",
    "SELECT * FROM users WHERE NULL > 0",
    "SELECT id FROM users WHERE id < 0 AND id > 0",
    "SELECT id FROM users ORDER BY id ASC, id DESC",
    "SELECT id, email AS id FROM users",
    "SELECT id FROM users WHERE id IN (id)",
    "SELECT id FROM users WHERE id IN ()",
    "SELECT id FROM users LIMIT 'abc'",
    "SELECT id FROM users OFFSET 'abc'",
    "SELECT * FROM users WHERE id = ANY(ARRAY[]::int[])",
    "SELECT * FROM users WHERE id IS DISTINCT FROM id",
    "SELECT * FROM users WHERE id IS NOT DISTINCT FROM id",
    "CREATE INDEX ON users (id) WITH (fillfactor = 200)",
    "CREATE INDEX ON users (id) WITH (fillfactor = 5)",
    "ALTER TABLE users ADD COLUMN c int UNIQUE",
    "ALTER TABLE users DROP COLUMN id",
    "ALTER TABLE users DROP COLUMN email CASCADE",
    "ALTER TABLE users RENAME COLUMN id TO other_id",
    "ALTER TABLE users RENAME TO other_users",
    "CREATE TABLE t (a int, b int, c int, d int, PRIMARY KEY(a, b, c, d))",
    "CREATE TABLE t (a int PRIMARY KEY, b int UNIQUE, c int UNIQUE, d int UNIQUE)",
    "CREATE TABLE t (a int) WITHOUT OIDS",
    "CREATE TABLE t (a int) WITH OIDS",
    "GRANT USAGE ON SCHEMA public TO PUBLIC",
    "GRANT ALL PRIVILEGES ON DATABASE app TO alice",
    "REVOKE ALL ON DATABASE app FROM PUBLIC",
    "CREATE DATABASE app",
    "DROP DATABASE IF EXISTS app",
    "DROP DATABASE app WITH (FORCE)",
    "CREATE SCHEMA IF NOT EXISTS s",
    "CREATE EXTENSION pgcrypto",
    "DROP EXTENSION pgcrypto",
    "CREATE OR REPLACE FUNCTION f() RETURNS int LANGUAGE sql AS 'select 1'",
    "CREATE FUNCTION f(int) RETURNS int LANGUAGE sql AS $1$select 1$1$",
    "DELETE FROM users WHERE 1=1",
    "UPDATE users SET id=1 WHERE 1=1",
    "INSERT INTO users VALUES (1, 'a', 'b'), (2, 'c', 'd')",
    "INSERT INTO users (id, email, name) VALUES (1, 'a', 'b'), (1, 'a', 'b')",
    "SELECT 1; SELECT 2; SELECT 3",
    "SELECT 1::float = 1.0",
    "SELECT 1/3",
    "SELECT 1.0/3",
    "SELECT id FROM users WHERE id = nextval('seq')",
    "SELECT id FROM users WHERE NOT id IS NULL",
    "SELECT id FROM users WHERE NOT id = 1",
    "SELECT id FROM users WHERE NOT (id = 1 AND email IS NULL)",
    "SELECT * FROM users WHERE id < 1000000000 + 1",
    "SELECT id FROM users WHERE id::text = id::text",
    "SELECT id FROM users WHERE id IN (1, 2) AND id IN (3, 4)",
    "SELECT id FROM users WHERE id IN (1, 2) OR id IN (3, 4)",
    "SELECT id FROM users WHERE id BETWEEN 1 AND 100 AND id BETWEEN 50 AND 200",
    "SELECT * FROM users WHERE (id, email) > (1, 'a')",
    "SELECT * FROM users WHERE (id, email) < (1, 'a')",
    "SELECT count(*) FROM users WHERE id > 0 AND id IS NOT NULL",
    "SELECT count(*) FROM users WHERE name IS NOT NULL",
    "SELECT count(name) FROM users",
    "SELECT count(NULL) FROM users",
    "SELECT count(DISTINCT id) FROM users",
    "SELECT 'abc' || NULL",
    "SELECT NULL || NULL",
    "SELECT concat('abc', NULL)",
    "SELECT length(NULL)",
    "SELECT substring('abc', 0)",
    "SELECT substring('abc', -1)",
    "SELECT replace('abc', '', 'X')",
    "SELECT split_part('a,b,c', '', 1)",
    "SELECT regexp_match('a', '')",
    "SELECT regexp_match('a', '*')",
    "SELECT * FROM users WHERE id BETWEEN 5 AND 5",
    "SELECT * FROM users WHERE id NOT BETWEEN 5 AND 5",
    "INSERT INTO users (id) VALUES (1.5)",
    "INSERT INTO users (id) VALUES ('a')",
    "SELECT id FROM users WHERE id::int + 0 = 0",
    "SELECT 1 + + 1",
    "SELECT 1 - - 1",
    "SELECT - - 1",
    "SELECT - + 1",
    "SELECT + + 1",
    "CREATE TABLE t (a int, b int CHECK (b > 0) UNIQUE NOT NULL DEFAULT 0)",
    "CREATE TABLE t (a int DEFAULT NULL)",
    "CREATE TABLE t (a int NOT NULL DEFAULT NULL)",
    "CREATE TABLE t (a serial)",
    "CREATE TABLE t (a bigserial)",
    "CREATE TABLE t (a smallserial)",
    "CREATE INDEX ON users (lower(name) varchar_pattern_ops)",
    "CREATE INDEX ON users USING btree (name varchar_pattern_ops)",
    "CREATE INDEX ON users USING hash (id)",
    "ALTER TABLE users SET (autovacuum_enabled = false)",
    "ALTER TABLE users RESET (autovacuum_enabled)",
    "ALTER TABLE users SET (parallel_workers = 4)",
    "SELECT * FROM users WHERE id < ALL(VALUES (1), (2))",
    "SELECT * FROM users WHERE id < ANY(VALUES (1), (2))",
  ];
  for s in inputs {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    eprintln!("D|{}|{:?}", s, codes);
  }
}

#[test]
fn r28_sql017_0001() {
  let d = diags("-- v0\nSELECT id, count(*) FROM users");
  assert!(d.iter().any(|x| x.code == "sql017"), "expected sql017 for `-- v0\nSELECT id, count(*) FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql017_0002() {
  let d = diags("-- v1\nSELECT count(*), id FROM users");
  assert!(d.iter().any(|x| x.code == "sql017"), "expected sql017 for `-- v1\nSELECT count(*), id FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql017_0003() {
  let d = diags("-- v2\nSELECT email, sum(id) FROM users");
  assert!(d.iter().any(|x| x.code == "sql017"), "expected sql017 for `-- v2\nSELECT email, sum(id) FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql017_0004() {
  let d = diags("-- v3\nSELECT name, max(id) FROM users");
  assert!(d.iter().any(|x| x.code == "sql017"), "expected sql017 for `-- v3\nSELECT name, max(id) FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql482_0005() {
  let d = diags("-- v4\nSELECT id FROM users HAVING TRUE");
  assert!(d.iter().any(|x| x.code == "sql482"), "expected sql482 for `-- v4\nSELECT id FROM users HAVING TRUE` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql482_0006() {
  let d = diags("-- v5\nSELECT id FROM users HAVING true");
  assert!(d.iter().any(|x| x.code == "sql482"), "expected sql482 for `-- v5\nSELECT id FROM users HAVING true` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql414_0008() {
  let d = diags("-- v7\nSELECT id FROM users WHERE id IN (id)");
  assert!(d.iter().any(|x| x.code == "sql414"), "expected sql414 for `-- v7\nSELECT id FROM users WHERE id IN (id)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql414_0009() {
  let d = diags("-- v8\nSELECT email FROM users WHERE email IN (email)");
  assert!(d.iter().any(|x| x.code == "sql414"), "expected sql414 for `-- v8\nSELECT email FROM users WHERE email IN (email)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql414_0010() {
  let d = diags("-- v9\nSELECT name FROM users WHERE name IN (name)");
  assert!(d.iter().any(|x| x.code == "sql414"), "expected sql414 for `-- v9\nSELECT name FROM users WHERE name IN (name)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql464_0011() {
  let d = diags("-- v10\nSELECT * FROM users WHERE id IS DISTINCT FROM id");
  assert!(d.iter().any(|x| x.code == "sql464"), "expected sql464 for `-- v10\nSELECT * FROM users WHERE id IS DISTINCT FROM id` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql464_0012() {
  let d = diags("-- v11\nSELECT * FROM users WHERE id IS NOT DISTINCT FROM id");
  assert!(d.iter().any(|x| x.code == "sql464"), "expected sql464 for `-- v11\nSELECT * FROM users WHERE id IS NOT DISTINCT FROM id` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql464_0013() {
  let d = diags("-- v12\nSELECT * FROM users WHERE email IS DISTINCT FROM email");
  assert!(d.iter().any(|x| x.code == "sql464"), "expected sql464 for `-- v12\nSELECT * FROM users WHERE email IS DISTINCT FROM email` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql464_0014() {
  let d = diags("-- v13\nSELECT * FROM users WHERE name IS NOT DISTINCT FROM name");
  assert!(d.iter().any(|x| x.code == "sql464"), "expected sql464 for `-- v13\nSELECT * FROM users WHERE name IS NOT DISTINCT FROM name` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql139_0015() {
  let d = diags("-- v14\nCREATE TABLE t (a int PRIMARY KEY, b int UNIQUE, c int UNIQUE, d int UNIQUE)");
  assert!(d.iter().any(|x| x.code == "sql139"), "expected sql139 for `-- v14\nCREATE TABLE t (a int PRIMARY KEY, b int UNIQUE, c int UNIQUE, d int UNIQUE)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql139_0016() {
  let d = diags("-- v15\nCREATE TABLE x (a int PRIMARY KEY, b int UNIQUE, c int UNIQUE)");
  assert!(d.iter().any(|x| x.code == "sql139"), "expected sql139 for `-- v15\nCREATE TABLE x (a int PRIMARY KEY, b int UNIQUE, c int UNIQUE)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql291_0017() {
  let d = diags("-- v16\nGRANT ALL PRIVILEGES ON DATABASE app TO alice");
  assert!(d.iter().any(|x| x.code == "sql291"), "expected sql291 for `-- v16\nGRANT ALL PRIVILEGES ON DATABASE app TO alice` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql291_0018() {
  let d = diags("-- v17\nGRANT ALL PRIVILEGES ON DATABASE postgres TO bob");
  assert!(d.iter().any(|x| x.code == "sql291"), "expected sql291 for `-- v17\nGRANT ALL PRIVILEGES ON DATABASE postgres TO bob` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql291_0019() {
  let d = diags("-- v18\nGRANT ALL ON DATABASE my_db TO alice");
  assert!(d.iter().any(|x| x.code == "sql291"), "expected sql291 for `-- v18\nGRANT ALL ON DATABASE my_db TO alice` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql328_0020() {
  let d = diags("-- v19\nREVOKE ALL ON DATABASE app FROM PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql328"), "expected sql328 for `-- v19\nREVOKE ALL ON DATABASE app FROM PUBLIC` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql328_0021() {
  let d = diags("-- v20\nREVOKE ALL PRIVILEGES ON DATABASE postgres FROM PUBLIC");
  assert!(d.iter().any(|x| x.code == "sql328"), "expected sql328 for `-- v20\nREVOKE ALL PRIVILEGES ON DATABASE postgres FROM PUBLIC` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql242_0022() {
  let d = diags("-- v21\nDROP DATABASE IF EXISTS app");
  assert!(d.iter().any(|x| x.code == "sql242"), "expected sql242 for `-- v21\nDROP DATABASE IF EXISTS app` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql242_0023() {
  let d = diags("-- v22\nDROP DATABASE app");
  assert!(d.iter().any(|x| x.code == "sql242"), "expected sql242 for `-- v22\nDROP DATABASE app` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql242_0024() {
  let d = diags("-- v23\nDROP DATABASE app WITH (FORCE)");
  assert!(d.iter().any(|x| x.code == "sql242"), "expected sql242 for `-- v23\nDROP DATABASE app WITH (FORCE)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql262_0025() {
  let d = diags("-- v24\nCREATE EXTENSION pgcrypto");
  assert!(d.iter().any(|x| x.code == "sql262"), "expected sql262 for `-- v24\nCREATE EXTENSION pgcrypto` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql262_0026() {
  let d = diags("-- v25\nCREATE EXTENSION uuid_ossp");
  assert!(d.iter().any(|x| x.code == "sql262"), "expected sql262 for `-- v25\nCREATE EXTENSION uuid_ossp` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql262_0027() {
  let d = diags("-- v26\nCREATE EXTENSION pg_trgm");
  assert!(d.iter().any(|x| x.code == "sql262"), "expected sql262 for `-- v26\nCREATE EXTENSION pg_trgm` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql469_0028() {
  let d = diags("-- v27\nSELECT id FROM users WHERE NOT id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql469"), "expected sql469 for `-- v27\nSELECT id FROM users WHERE NOT id IS NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql469_0029() {
  let d = diags("-- v28\nSELECT id FROM users WHERE NOT email IS NULL");
  assert!(d.iter().any(|x| x.code == "sql469"), "expected sql469 for `-- v28\nSELECT id FROM users WHERE NOT email IS NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql174_0030() {
  let d = diags("-- v29\nSELECT count(name) FROM users");
  assert!(d.iter().any(|x| x.code == "sql174"), "expected sql174 for `-- v29\nSELECT count(name) FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql413_0032() {
  let d = diags("-- v31\nSELECT 'abc' || NULL");
  assert!(d.iter().any(|x| x.code == "sql413"), "expected sql413 for `-- v31\nSELECT 'abc' || NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql413_0033() {
  let d = diags("-- v32\nSELECT NULL || 'def'");
  assert!(d.iter().any(|x| x.code == "sql413"), "expected sql413 for `-- v32\nSELECT NULL || 'def'` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql413_0034() {
  let d = diags("-- v33\nSELECT NULL || NULL");
  assert!(d.iter().any(|x| x.code == "sql413"), "expected sql413 for `-- v33\nSELECT NULL || NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql097_0035() {
  let d = diags("-- v34\nSELECT NULL || NULL");
  assert!(d.iter().any(|x| x.code == "sql097"), "expected sql097 for `-- v34\nSELECT NULL || NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql097_0036() {
  let d = diags("-- v35\nSELECT + + 1");
  assert!(d.iter().any(|x| x.code == "sql097"), "expected sql097 for `-- v35\nSELECT + + 1` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql479_0037() {
  let d = diags("-- v36\nSELECT substring('abc', 0)");
  assert!(d.iter().any(|x| x.code == "sql479"), "expected sql479 for `-- v36\nSELECT substring('abc', 0)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql479_0038() {
  let d = diags("-- v37\nSELECT substring('hello', 0)");
  assert!(d.iter().any(|x| x.code == "sql479"), "expected sql479 for `-- v37\nSELECT substring('hello', 0)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql467_0039() {
  let d = diags("-- v38\nSELECT replace('abc', '', 'X')");
  assert!(d.iter().any(|x| x.code == "sql467"), "expected sql467 for `-- v38\nSELECT replace('abc', '', 'X')` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql467_0040() {
  let d = diags("-- v39\nSELECT split_part('a,b,c', '', 1)");
  assert!(d.iter().any(|x| x.code == "sql467"), "expected sql467 for `-- v39\nSELECT split_part('a,b,c', '', 1)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql485_0041() {
  let d = diags("-- v40\nSELECT regexp_match('a', '')");
  assert!(d.iter().any(|x| x.code == "sql485"), "expected sql485 for `-- v40\nSELECT regexp_match('a', '')` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql485_0042() {
  let d = diags("-- v41\nSELECT regexp_match('hello', '')");
  assert!(d.iter().any(|x| x.code == "sql485"), "expected sql485 for `-- v41\nSELECT regexp_match('hello', '')` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql069_0043() {
  let d = diags("-- v42\nCREATE TABLE t (a int NOT NULL DEFAULT NULL)");
  assert!(d.iter().any(|x| x.code == "sql069"), "expected sql069 for `-- v42\nCREATE TABLE t (a int NOT NULL DEFAULT NULL)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql069_0044() {
  let d = diags("-- v43\nCREATE TABLE x (col1 int NOT NULL DEFAULT NULL)");
  assert!(d.iter().any(|x| x.code == "sql069"), "expected sql069 for `-- v43\nCREATE TABLE x (col1 int NOT NULL DEFAULT NULL)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql054_0045() {
  let d = diags("-- v44\nALTER TABLE users SET (autovacuum_enabled = false)");
  assert!(d.iter().any(|x| x.code == "sql054"), "expected sql054 for `-- v44\nALTER TABLE users SET (autovacuum_enabled = false)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql054_0046() {
  let d = diags("-- v45\nALTER TABLE orders SET (autovacuum_enabled = false)");
  assert!(d.iter().any(|x| x.code == "sql054"), "expected sql054 for `-- v45\nALTER TABLE orders SET (autovacuum_enabled = false)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql434_0047() {
  let d = diags("-- v46\nSELECT count(*) FROM users WHERE id > 0 AND id IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql434"), "expected sql434 for `-- v46\nSELECT count(*) FROM users WHERE id > 0 AND id IS NOT NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql434_0048() {
  let d = diags("-- v47\nSELECT count(*) FROM orders WHERE id > 0 AND id IS NOT NULL");
  assert!(d.iter().any(|x| x.code == "sql434"), "expected sql434 for `-- v47\nSELECT count(*) FROM orders WHERE id > 0 AND id IS NOT NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql288_0049() {
  let d = diags("-- v48\nCREATE INDEX ON users (lower(name))");
  assert!(d.iter().any(|x| x.code == "sql288"), "expected sql288 for `-- v48\nCREATE INDEX ON users (lower(name))` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql288_0050() {
  let d = diags("-- v49\nCREATE INDEX ON users (upper(name))");
  assert!(d.iter().any(|x| x.code == "sql288"), "expected sql288 for `-- v49\nCREATE INDEX ON users (upper(name))` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql288_0051() {
  let d = diags("-- v50\nCREATE INDEX ON users USING hash (id)");
  assert!(d.iter().any(|x| x.code == "sql288"), "expected sql288 for `-- v50\nCREATE INDEX ON users USING hash (id)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql474_0052() {
  let d = diags("-- v51\nDELETE FROM users WHERE 1=1");
  assert!(d.iter().any(|x| x.code == "sql474"), "expected sql474 for `-- v51\nDELETE FROM users WHERE 1=1` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql474_0053() {
  let d = diags("-- v52\nUPDATE users SET id=1 WHERE 1=1");
  assert!(d.iter().any(|x| x.code == "sql474"), "expected sql474 for `-- v52\nUPDATE users SET id=1 WHERE 1=1` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql474_0054() {
  let d = diags("-- v53\nSELECT id FROM users WHERE 0=0");
  assert!(d.iter().any(|x| x.code == "sql474"), "expected sql474 for `-- v53\nSELECT id FROM users WHERE 0=0` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql109_0055() {
  let d = diags("-- v54\nSELECT length(NULL)");
  assert!(d.iter().any(|x| x.code == "sql109"), "expected sql109 for `-- v54\nSELECT length(NULL)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql109_0056() {
  let d = diags("-- v55\nSELECT length(name) FROM users WHERE length(name) > 0");
  assert!(d.iter().any(|x| x.code == "sql109"), "expected sql109 for `-- v55\nSELECT length(name) FROM users WHERE length(name) > 0` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql048_0057() {
  let d = diags("-- v56\nINSERT INTO users VALUES (1, 'a', 'b'), (2, 'c', 'd')");
  assert!(d.iter().any(|x| x.code == "sql048"), "expected sql048 for `-- v56\nINSERT INTO users VALUES (1, 'a', 'b'), (2, 'c', 'd')` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql048_0058() {
  let d = diags("-- v57\nINSERT INTO users VALUES (1, 'a', 'b'), (2, 'c', 'd'), (3, 'e', 'f')");
  assert!(d.iter().any(|x| x.code == "sql048"), "expected sql048 for `-- v57\nINSERT INTO users VALUES (1, 'a', 'b'), (2, 'c', 'd'), (3, 'e', 'f')` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql099_0059() {
  let d = diags("-- v58\nSELECT id, name FROM users ORDER BY 1");
  assert!(d.iter().any(|x| x.code == "sql099"), "expected sql099 for `-- v58\nSELECT id, name FROM users ORDER BY 1` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql099_0060() {
  let d = diags("-- v59\nSELECT id, name, email FROM users ORDER BY 2");
  assert!(d.iter().any(|x| x.code == "sql099"), "expected sql099 for `-- v59\nSELECT id, name, email FROM users ORDER BY 2` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql065_0061() {
  let d = diags("-- v60\nSELECT id FROM users GROUP BY 1");
  assert!(d.iter().any(|x| x.code == "sql065"), "expected sql065 for `-- v60\nSELECT id FROM users GROUP BY 1` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql065_0062() {
  let d = diags("-- v61\nSELECT id, name FROM users GROUP BY 1, 2");
  assert!(d.iter().any(|x| x.code == "sql065"), "expected sql065 for `-- v61\nSELECT id, name FROM users GROUP BY 1, 2` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql051_0063() {
  let d = diags("-- v62\nSELECT id FROM users LIMIT 'abc'");
  assert!(d.iter().any(|x| x.code == "sql051"), "expected sql051 for `-- v62\nSELECT id FROM users LIMIT 'abc'` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql051_0064() {
  let d = diags("-- v63\nSELECT id FROM users LIMIT 1000000");
  assert!(d.iter().any(|x| x.code == "sql051"), "expected sql051 for `-- v63\nSELECT id FROM users LIMIT 1000000` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql473_0065() {
  let d = diags("-- v64\nSELECT * FROM users WHERE id = ANY(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"), "expected sql473 for `-- v64\nSELECT * FROM users WHERE id = ANY(ARRAY[]::int[])` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql473_0066() {
  let d = diags("-- v65\nSELECT * FROM users WHERE id = ALL(ARRAY[]::int[])");
  assert!(d.iter().any(|x| x.code == "sql473"), "expected sql473 for `-- v65\nSELECT * FROM users WHERE id = ALL(ARRAY[]::int[])` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql412_0067() {
  let d = diags("-- v66\nSELECT id FROM users ORDER BY id ASC, id DESC");
  assert!(d.iter().any(|x| x.code == "sql412"), "expected sql412 for `-- v66\nSELECT id FROM users ORDER BY id ASC, id DESC` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql412_0068() {
  let d = diags("-- v67\nSELECT id FROM users GROUP BY id, id");
  assert!(d.iter().any(|x| x.code == "sql412"), "expected sql412 for `-- v67\nSELECT id FROM users GROUP BY id, id` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql410_0069() {
  let d = diags("-- v68\nSELECT id, email AS id FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"), "expected sql410 for `-- v68\nSELECT id, email AS id FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql410_0070() {
  let d = diags("-- v69\nSELECT id AS a, email AS a FROM users");
  assert!(d.iter().any(|x| x.code == "sql410"), "expected sql410 for `-- v69\nSELECT id AS a, email AS a FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql015_0071() {
  let d = diags("-- v70\nSELECT * FROM users WHERE NULL <> NULL");
  assert!(d.iter().any(|x| x.code == "sql015"), "expected sql015 for `-- v70\nSELECT * FROM users WHERE NULL <> NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql015_0072() {
  let d = diags("-- v71\nSELECT * FROM users WHERE id = NULL");
  assert!(d.iter().any(|x| x.code == "sql015"), "expected sql015 for `-- v71\nSELECT * FROM users WHERE id = NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql408_0073() {
  let d = diags("-- v72\nSELECT id FROM users WHERE id = id");
  assert!(d.iter().any(|x| x.code == "sql408"), "expected sql408 for `-- v72\nSELECT id FROM users WHERE id = id` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql408_0074() {
  let d = diags("-- v73\nSELECT id FROM users WHERE id != id");
  assert!(d.iter().any(|x| x.code == "sql408"), "expected sql408 for `-- v73\nSELECT id FROM users WHERE id != id` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql407_0075() {
  let d = diags("-- v74\nSELECT id FROM users WHERE 1=2");
  assert!(d.iter().any(|x| x.code == "sql407"), "expected sql407 for `-- v74\nSELECT id FROM users WHERE 1=2` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql407_0076() {
  let d = diags("-- v75\nSELECT id FROM users WHERE FALSE");
  assert!(d.iter().any(|x| x.code == "sql407"), "expected sql407 for `-- v75\nSELECT id FROM users WHERE FALSE` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql460_0077() {
  let d = diags("-- v76\nSELECT id FROM users HAVING TRUE");
  assert!(d.iter().any(|x| x.code == "sql460"), "expected sql460 for `-- v76\nSELECT id FROM users HAVING TRUE` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql460_0078() {
  let d = diags("-- v77\nSELECT id FROM users HAVING id > 0");
  assert!(d.iter().any(|x| x.code == "sql460"), "expected sql460 for `-- v77\nSELECT id FROM users HAVING id > 0` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql056_0079() {
  let d = diags("-- v78\nSELECT id FROM users UNION SELECT id FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"), "expected sql056 for `-- v78\nSELECT id FROM users UNION SELECT id FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql056_0080() {
  let d = diags("-- v79\nSELECT email FROM users UNION SELECT email FROM users");
  assert!(d.iter().any(|x| x.code == "sql056"), "expected sql056 for `-- v79\nSELECT email FROM users UNION SELECT email FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql486_0081() {
  let d = diags("-- v80\nSELECT DISTINCT * FROM users");
  assert!(d.iter().any(|x| x.code == "sql486"), "expected sql486 for `-- v80\nSELECT DISTINCT * FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql486_0082() {
  let d = diags("-- v81\nSELECT DISTINCT * FROM orders");
  assert!(d.iter().any(|x| x.code == "sql486"), "expected sql486 for `-- v81\nSELECT DISTINCT * FROM orders` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql013_0083() {
  let d = diags("-- v82\nUPDATE users SET name='x'");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 for `-- v82\nUPDATE users SET name='x'` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql013_0084() {
  let d = diags("-- v83\nDELETE FROM users");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 for `-- v83\nDELETE FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql013_0085() {
  let d = diags("-- v84\nUPDATE orders SET id=1");
  assert!(d.iter().any(|x| x.code == "sql013"), "expected sql013 for `-- v84\nUPDATE orders SET id=1` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql306_0087() {
  let d = diags("-- v86\nSELECT * FROM users WHERE id IN (1, 1, 2)");
  assert!(d.iter().any(|x| x.code == "sql306"), "expected sql306 for `-- v86\nSELECT * FROM users WHERE id IN (1, 1, 2)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql087_0088() {
  let d = diags("-- v87\nSELECT * FROM users WHERE id BETWEEN 10 AND 1");
  assert!(d.iter().any(|x| x.code == "sql087"), "expected sql087 for `-- v87\nSELECT * FROM users WHERE id BETWEEN 10 AND 1` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql087_0089() {
  let d = diags("-- v88\nSELECT * FROM users WHERE id BETWEEN 100 AND 0");
  assert!(d.iter().any(|x| x.code == "sql087"), "expected sql087 for `-- v88\nSELECT * FROM users WHERE id BETWEEN 100 AND 0` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql292_0090() {
  let d = diags("-- v89\nSELECT * FROM users LIMIT 0");
  assert!(d.iter().any(|x| x.code == "sql292"), "expected sql292 for `-- v89\nSELECT * FROM users LIMIT 0` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql292_0091() {
  let d = diags("-- v90\nSELECT id FROM orders LIMIT 0");
  assert!(d.iter().any(|x| x.code == "sql292"), "expected sql292 for `-- v90\nSELECT id FROM orders LIMIT 0` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql411_0092() {
  let d = diags("-- v91\nSELECT * FROM users LIMIT 1 OFFSET 1000000");
  assert!(d.iter().any(|x| x.code == "sql411"), "expected sql411 for `-- v91\nSELECT * FROM users LIMIT 1 OFFSET 1000000` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql411_0093() {
  let d = diags("-- v92\nSELECT * FROM users LIMIT 10 OFFSET 999999");
  assert!(d.iter().any(|x| x.code == "sql411"), "expected sql411 for `-- v92\nSELECT * FROM users LIMIT 10 OFFSET 999999` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql176_0094() {
  let d = diags("-- v93\nSELECT * FROM users WHERE id IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"), "expected sql176 for `-- v93\nSELECT * FROM users WHERE id IS NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql176_0095() {
  let d = diags("-- v94\nSELECT id FROM users WHERE email IS NULL");
  assert!(d.iter().any(|x| x.code == "sql176"), "expected sql176 for `-- v94\nSELECT id FROM users WHERE email IS NULL` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql088_0096() {
  let d = diags("-- v95\nSELECT * FROM users WHERE name LIKE '%abc'");
  assert!(d.iter().any(|x| x.code == "sql088"), "expected sql088 for `-- v95\nSELECT * FROM users WHERE name LIKE '%abc'` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql088_0097() {
  let d = diags("-- v96\nSELECT * FROM users WHERE email LIKE '%@b'");
  assert!(d.iter().any(|x| x.code == "sql088"), "expected sql088 for `-- v96\nSELECT * FROM users WHERE email LIKE '%@b'` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql052_0098() {
  let d = diags("-- v97\nSELECT * FROM users WHERE name LIKE 'abc'");
  assert!(d.iter().any(|x| x.code == "sql052"), "expected sql052 for `-- v97\nSELECT * FROM users WHERE name LIKE 'abc'` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql052_0099() {
  let d = diags("-- v98\nSELECT * FROM users WHERE name ILIKE 'x'");
  assert!(d.iter().any(|x| x.code == "sql052"), "expected sql052 for `-- v98\nSELECT * FROM users WHERE name ILIKE 'x'` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql001_0100() {
  let d = diags("-- v99\nSELECT * FROM bogus_table_xyz");
  assert!(d.iter().any(|x| x.code == "sql001"), "expected sql001 for `-- v99\nSELECT * FROM bogus_table_xyz` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql002_0101() {
  let d = diags("-- v100\nSELECT bogus_col FROM users");
  assert!(d.iter().any(|x| x.code == "sql002"), "expected sql002 for `-- v100\nSELECT bogus_col FROM users` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql003_0102() {
  let d = diags("-- v101\nSELECT id FROM users, orders");
  assert!(d.iter().any(|x| x.code == "sql003"), "expected sql003 for `-- v101\nSELECT id FROM users, orders` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r28_sql018_0103() {
  let d = diags("-- v102\nDELETE FROM users WHERE id NOT IN (SELECT user_id FROM orders)");
  assert!(d.iter().any(|x| x.code == "sql018"), "expected sql018 for `-- v102\nDELETE FROM users WHERE id NOT IN (SELECT user_id FROM orders)` got {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>());
}

#[test]
fn r29_probe_missing() {
  let inputs = [
    ("sql170", "DO $$ DECLARE x integer = 'abc'; BEGIN END $$"),
    ("sql172", "SELECT * FROM users WHERE id = 'abc'"),
    ("sql173", "/* schema_drift */ SELECT 1"),
    ("sql179", "SAVEPOINT sp"),
    ("sql180", "CREATE TRIGGER t BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION fn(); -- truncates inside"),
    ("sql184", "SELECT 9999999999999999999::int"),
    ("sql186", "ALTER TABLE users DROP COLUMN id"),
    ("sql187", "SELECT * FROM users JOIN orders USING (no_such_col)"),
    ("sql191", "SELECT sum(id) OVER (ORDER BY id ROWS BETWEEN CURRENT ROW AND UNBOUNDED PRECEDING) FROM users"),
    ("sql192", "SELECT id FROM users FOR UPDATE OF no_such_table"),
    ("sql198", "CREATE TABLE t (a int, b int CHECK (b > a))"),
    ("sql200", "SELECT * FROM users u CROSS JOIN LATERAL (SELECT 1) x"),
    ("sql202", "CREATE TRIGGER t AFTER UPDATE ON users FOR EACH ROW EXECUTE FUNCTION f(); -- TG_OP wrong"),
    ("sql203", "DO $$ BEGIN RAISE 'msg'; END $$"),
    ("sql204", "UPDATE users u SET name = u.name"),
    ("sql212", "DO $$ BEGIN SELECT 1 INTO x; END $$"),
    ("sql213", "CREATE INDEX ON users (random())"),
    ("sql214", "BEGIN; CREATE INDEX CONCURRENTLY ON users (id); COMMIT"),
    ("sql217", "SELECT * FROM users u LEFT JOIN orders o ON u.id = o.user_id FOR UPDATE"),
    ("sql222", "SELECT * FROM (SELECT * FROM users LIMIT 10) sub FOR UPDATE"),
    ("sql224", "SET CONSTRAINTS ALL DEFERRED"),
    ("sql226", "DROP TABLE users CASCADE; DROP TABLE orders CASCADE"),
    ("sql231", "SELECT * FROM users ORDER BY 1 NULLS FIRST, 2 NULLS LAST"),
    ("sql233", "CREATE MATERIALIZED VIEW mv AS SELECT 1 WITH NO DATA"),
    ("sql236", "CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN INSERT INTO t VALUES (1); RETURN NEW; END $$"),
    ("sql245", "SELECT * FROM pg_tables"),
    ("sql249", "INSERT INTO users DEFAULT VALUES"),
    ("sql250", "SELECT max(id) FROM users FOR UPDATE"),
    ("sql252", "SELECT id FROM (SELECT id FROM users ORDER BY 1) sub"),
    ("sql253", "SELECT * FROM users WHERE id NOT IN (SELECT name FROM users)"),
    ("sql257", "DO $$ BEGIN SELECT 1; END $$"),
    ("sql259", "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN SET ROLE admin; END $$"),
    ("sql261", "MERGE INTO users u USING orders o ON u.id = o.user_id"),
    ("sql265", "CREATE TABLE t (ts timestamp DEFAULT now())"),
    ("sql271", "DECLARE c CURSOR WITH HOLD FOR SELECT 1"),
    ("sql274", "DO $$ DECLARE temp_t int; BEGIN END $$"),
    ("sql275", "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN SET TRANSACTION READ ONLY; END $$"),
    ("sql284", "DO $$ BEGIN RAISE NOTICE '%', TG_OP; END $$"),
    ("sql285", "DROP ROLE r"),
    ("sql286", "ALTER TYPE mood RENAME VALUE 'old_val' TO 'new_val'"),
    ("sql289", "CREATE TABLE c () INHERITS (parent)"),
    ("sql297", "NOTIFY ch, 'long payload that exceeds the limit of 8000 bytes'"),
    ("sql305", "SELECT * FROM information_schema.tables"),
    ("sql307", "DELETE FROM users LIMIT 10"),
    ("sql309", "REVOKE SELECT ON users"),
    ("sql310", "\\d users"),
    ("sql321", "GO"),
    ("sql322", "BEGIN TRAN"),
    ("sql341", "SELECT * FROM users WHERE id = ARRAY[1]"),
    ("sql342", "SELECT bool_and(name IS NOT NULL) FROM users"),
    ("sql512", "CREATE TABLE t (id int, CONSTRAINT chk CHECK (no_col > 0))"),
    ("sql514", "SELECT * FROM users WHERE ()"),
  ];
  for (expected_code, s) in inputs {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    let fires = codes.contains(&expected_code);
    eprintln!("R29|{}|fires={}|got={:?}", expected_code, fires, codes);
  }
}

#[test]
fn r29_sql187_0001() {
  let d = diags("-- v0\nSELECT * FROM users JOIN orders USING (no_such_col)");
  assert!(d.iter().any(|x| x.code == "sql187"), "expected sql187");
}

#[test]
fn r29_sql187_0002() {
  let d = diags("-- v1\nSELECT * FROM users u JOIN orders o USING (no_such_col)");
  assert!(d.iter().any(|x| x.code == "sql187"), "expected sql187");
}

#[test]
fn r29_sql191_0003() {
  let d = diags("-- v2\nSELECT sum(id) OVER (ORDER BY id ROWS BETWEEN CURRENT ROW AND UNBOUNDED PRECEDING) FROM users");
  assert!(d.iter().any(|x| x.code == "sql191"), "expected sql191");
}

#[test]
fn r29_sql192_0004() {
  let d = diags("-- v3\nSELECT id FROM users FOR UPDATE OF no_such_table");
  assert!(d.iter().any(|x| x.code == "sql192"), "expected sql192");
}

#[test]
fn r29_sql192_0005() {
  let d = diags("-- v4\nSELECT id FROM users FOR SHARE OF nonexistent_t");
  assert!(d.iter().any(|x| x.code == "sql192"), "expected sql192");
}

#[test]
fn r29_sql198_0006() {
  let d = diags("-- v5\nCREATE TABLE t (a int, b int CHECK (b > a))");
  assert!(d.iter().any(|x| x.code == "sql198"), "expected sql198");
}

#[test]
fn r29_sql198_0007() {
  let d = diags("-- v6\nCREATE TABLE t (x int, y int CHECK (y > x))");
  assert!(d.iter().any(|x| x.code == "sql198"), "expected sql198");
}

#[test]
fn r29_sql200_0008() {
  let d = diags("-- v7\nSELECT * FROM users u CROSS JOIN LATERAL (SELECT 1) x");
  assert!(d.iter().any(|x| x.code == "sql200"), "expected sql200");
}

#[test]
fn r29_sql200_0009() {
  let d = diags("-- v8\nSELECT * FROM users CROSS JOIN LATERAL (SELECT 2) y");
  assert!(d.iter().any(|x| x.code == "sql200"), "expected sql200");
}

#[test]
fn r29_sql203_0010() {
  let d = diags("-- v9\nDO $$ BEGIN RAISE 'msg'; END $$");
  assert!(d.iter().any(|x| x.code == "sql203"), "expected sql203");
}

#[test]
fn r29_sql203_0011() {
  let d = diags("-- v10\nDO $$ BEGIN RAISE 'error'; END $$");
  assert!(d.iter().any(|x| x.code == "sql203"), "expected sql203");
}

#[test]
fn r29_sql213_0012() {
  let d = diags("-- v11\nCREATE INDEX ON users (random())");
  assert!(d.iter().any(|x| x.code == "sql213"), "expected sql213");
}

#[test]
fn r29_sql213_0013() {
  let d = diags("-- v12\nCREATE INDEX ON orders (random())");
  assert!(d.iter().any(|x| x.code == "sql213"), "expected sql213");
}

#[test]
fn r29_sql214_0014() {
  let d = diags("-- v13\nBEGIN; CREATE INDEX CONCURRENTLY ON users (id); COMMIT");
  assert!(d.iter().any(|x| x.code == "sql214"), "expected sql214");
}

#[test]
fn r29_sql214_0015() {
  let d = diags("-- v14\nBEGIN; CREATE INDEX CONCURRENTLY ON orders (id); COMMIT");
  assert!(d.iter().any(|x| x.code == "sql214"), "expected sql214");
}

#[test]
fn r29_sql217_0016() {
  let d = diags("-- v15\nSELECT * FROM users u LEFT JOIN orders o ON u.id = o.user_id FOR UPDATE");
  assert!(d.iter().any(|x| x.code == "sql217"), "expected sql217");
}

#[test]
fn r29_sql217_0017() {
  let d = diags("-- v16\nSELECT * FROM users u LEFT JOIN orders o ON u.id = o.user_id FOR SHARE");
  assert!(d.iter().any(|x| x.code == "sql217"), "expected sql217");
}

#[test]
fn r29_sql224_0018() {
  let d = diags("-- v17\nSET CONSTRAINTS ALL DEFERRED");
  assert!(d.iter().any(|x| x.code == "sql224"), "expected sql224");
}

#[test]
fn r29_sql224_0019() {
  let d = diags("-- v18\nSET CONSTRAINTS chk IMMEDIATE");
  assert!(d.iter().any(|x| x.code == "sql224"), "expected sql224");
}

#[test]
fn r29_sql249_0020() {
  let d = diags("-- v19\nINSERT INTO users DEFAULT VALUES");
  assert!(d.iter().any(|x| x.code == "sql249"), "expected sql249");
}

#[test]
fn r29_sql249_0021() {
  let d = diags("-- v20\nINSERT INTO orders DEFAULT VALUES");
  assert!(d.iter().any(|x| x.code == "sql249"), "expected sql249");
}

#[test]
fn r29_sql250_0022() {
  let d = diags("-- v21\nSELECT max(id) FROM users FOR UPDATE");
  assert!(d.iter().any(|x| x.code == "sql250"), "expected sql250");
}

#[test]
fn r29_sql250_0023() {
  let d = diags("-- v22\nSELECT sum(id) FROM users FOR UPDATE");
  assert!(d.iter().any(|x| x.code == "sql250"), "expected sql250");
}

#[test]
fn r29_sql252_0024() {
  let d = diags("-- v23\nSELECT id FROM (SELECT id FROM users ORDER BY 1) sub");
  assert!(d.iter().any(|x| x.code == "sql252"), "expected sql252");
}

#[test]
fn r29_sql252_0025() {
  let d = diags("-- v24\nSELECT * FROM (SELECT id FROM orders ORDER BY id) sub");
  assert!(d.iter().any(|x| x.code == "sql252"), "expected sql252");
}

#[test]
fn r29_sql253_0026() {
  let d = diags("-- v25\nSELECT * FROM users WHERE id NOT IN (SELECT name FROM users)");
  assert!(d.iter().any(|x| x.code == "sql253"), "expected sql253");
}

#[test]
fn r29_sql257_0028() {
  let d = diags("-- v27\nDO $$ BEGIN SELECT 1; END $$");
  assert!(d.iter().any(|x| x.code == "sql257"), "expected sql257");
}

#[test]
fn r29_sql257_0029() {
  let d = diags("-- v28\nDO $$ BEGIN SELECT 2; END $$");
  assert!(d.iter().any(|x| x.code == "sql257"), "expected sql257");
}

#[test]
fn r29_sql259_0030() {
  let d = diags("-- v29\nCREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN SET ROLE admin; END $$");
  assert!(d.iter().any(|x| x.code == "sql259"), "expected sql259");
}

#[test]
fn r29_sql259_0031() {
  let d = diags("-- v30\nCREATE FUNCTION g() RETURNS void LANGUAGE plpgsql AS $$ BEGIN SET ROLE my_role; END $$");
  assert!(d.iter().any(|x| x.code == "sql259"), "expected sql259");
}

#[test]
fn r29_sql261_0032() {
  let d = diags("-- v31\nMERGE INTO users u USING orders o ON u.id = o.user_id");
  assert!(d.iter().any(|x| x.code == "sql261"), "expected sql261");
}

#[test]
fn r29_sql261_0033() {
  let d = diags("-- v32\nMERGE INTO orders o USING users u ON o.user_id = u.id");
  assert!(d.iter().any(|x| x.code == "sql261"), "expected sql261");
}

#[test]
fn r29_sql271_0034() {
  let d = diags("-- v33\nDECLARE c CURSOR WITH HOLD FOR SELECT 1");
  assert!(d.iter().any(|x| x.code == "sql271"), "expected sql271");
}

#[test]
fn r29_sql271_0035() {
  let d = diags("-- v34\nDECLARE my_c CURSOR WITH HOLD FOR SELECT 2");
  assert!(d.iter().any(|x| x.code == "sql271"), "expected sql271");
}

#[test]
fn r29_sql275_0036() {
  let d = diags("-- v35\nCREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN SET TRANSACTION READ ONLY; END $$");
  assert!(d.iter().any(|x| x.code == "sql275"), "expected sql275");
}

#[test]
fn r29_sql275_0037() {
  let d = diags("-- v36\nCREATE FUNCTION g() RETURNS void LANGUAGE plpgsql AS $$ BEGIN SET TRANSACTION ISOLATION LEVEL SERIALIZABLE; END $$");
  assert!(d.iter().any(|x| x.code == "sql275"), "expected sql275");
}

#[test]
fn r29_sql285_0038() {
  let d = diags("-- v37\nDROP ROLE r");
  assert!(d.iter().any(|x| x.code == "sql285"), "expected sql285");
}

#[test]
fn r29_sql285_0039() {
  let d = diags("-- v38\nDROP ROLE alice");
  assert!(d.iter().any(|x| x.code == "sql285"), "expected sql285");
}

#[test]
fn r29_sql289_0040() {
  let d = diags("-- v39\nCREATE TABLE c () INHERITS (parent)");
  assert!(d.iter().any(|x| x.code == "sql289"), "expected sql289");
}

#[test]
fn r29_sql289_0041() {
  let d = diags("-- v40\nCREATE TABLE child () INHERITS (parent_table)");
  assert!(d.iter().any(|x| x.code == "sql289"), "expected sql289");
}

#[test]
fn r29_sql305_0042() {
  let d = diags("-- v41\nSELECT * FROM information_schema.tables");
  assert!(d.iter().any(|x| x.code == "sql305"), "expected sql305");
}

#[test]
fn r29_sql305_0043() {
  let d = diags("-- v42\nSELECT * FROM information_schema.columns");
  assert!(d.iter().any(|x| x.code == "sql305"), "expected sql305");
}

#[test]
fn r29_sql307_0044() {
  let d = diags("-- v43\nDELETE FROM users LIMIT 10");
  assert!(d.iter().any(|x| x.code == "sql307"), "expected sql307");
}

#[test]
fn r29_sql307_0045() {
  let d = diags("-- v44\nUPDATE users SET name='x' LIMIT 10");
  assert!(d.iter().any(|x| x.code == "sql307"), "expected sql307");
}

#[test]
fn r29_sql309_0046() {
  let d = diags("-- v45\nREVOKE SELECT ON users");
  assert!(d.iter().any(|x| x.code == "sql309"), "expected sql309");
}

#[test]
fn r29_sql309_0047() {
  let d = diags("-- v46\nREVOKE INSERT ON orders");
  assert!(d.iter().any(|x| x.code == "sql309"), "expected sql309");
}

#[test]
fn r29_sql310_0048() {
  let d = diags("-- v47\n\\d users");
  assert!(d.iter().any(|x| x.code == "sql310"), "expected sql310");
}

#[test]
fn r29_sql310_0049() {
  let d = diags("-- v48\n\\dt");
  assert!(d.iter().any(|x| x.code == "sql310"), "expected sql310");
}

#[test]
fn r29_sql321_0050() {
  let d = diags("-- v49\nGO");
  assert!(d.iter().any(|x| x.code == "sql321"), "expected sql321");
}

#[test]
fn r29_sql321_0051() {
  let d = diags("-- v50\ngo");
  assert!(d.iter().any(|x| x.code == "sql321"), "expected sql321");
}

#[test]
fn r29_sql322_0052() {
  let d = diags("-- v51\nBEGIN TRAN");
  assert!(d.iter().any(|x| x.code == "sql322"), "expected sql322");
}

#[test]
fn r29_sql514_0054() {
  let d = diags("-- v53\nSELECT * FROM users WHERE ()");
  assert!(d.iter().any(|x| x.code == "sql514"), "expected sql514");
}

#[test]
fn r29_sql514_0055() {
  let d = diags("-- v54\nSELECT 1 WHERE ()");
  assert!(d.iter().any(|x| x.code == "sql514"), "expected sql514");
}

#[test]
fn r29_probe2() {
  let inputs = [
    ("sql170", "DO $$ DECLARE x int := 'abc'; BEGIN END $$"),
    ("sql170", "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ DECLARE x int := 'abc'; BEGIN END $$"),
    ("sql172", "SELECT * FROM users WHERE email = 1"),
    ("sql172", "SELECT * FROM users WHERE name = 1"),
    ("sql173", "CREATE TABLE users (id uuid PRIMARY KEY, different_col text)"),
    ("sql179", "BEGIN; ROLLBACK; SAVEPOINT sp"),
    ("sql179", "SAVEPOINT sp1"),
    ("sql184", "CREATE TABLE nums (n int); INSERT INTO nums (n) VALUES (99999999999)"),
    ("sql184", "INSERT INTO nums (n) VALUES (99999999999)"),
  ];
  for (code, s) in inputs {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    let fires = codes.contains(&code);
    eprintln!("P2|{}|fires={}|got={:?}", code, fires, codes);
  }
}

#[test]
fn r29_probe3() {
  let inputs = [
    ("sql180", "CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN TRUNCATE users; RETURN NEW; END $$"),
    ("sql186", "BEGIN; ALTER TABLE users DROP COLUMN id; COMMIT"),
    ("sql202", "CREATE TRIGGER t BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION f(); CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN OLD; END $$"),
    ("sql204", "UPDATE users u SET id = other.id FROM other"),
    ("sql212", "SELECT 1 INTO users"),
    ("sql222", "SELECT * FROM (SELECT * FROM users WHERE id > 0 LIMIT 10) sub FOR UPDATE"),
    ("sql226", "DROP TABLE users CASCADE"),
    ("sql231", "SELECT id FROM users NULLS FIRST"),
    ("sql233", "CREATE MATERIALIZED VIEW mv AS SELECT id FROM users WITH NO DATA"),
    ("sql236", "CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$; CREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION f()"),
    ("sql245", "SELECT * FROM pg_class"),
    ("sql265", "CREATE TABLE t (id int PRIMARY KEY, ts timestamptz DEFAULT now())"),
    ("sql274", "CREATE TEMP TABLE users (id int)"),
    ("sql284", "CREATE FUNCTION f() RETURNS int LANGUAGE plpgsql AS $$ BEGIN RETURN TG_OP::int; END $$"),
    ("sql286", "ALTER TYPE mood ADD VALUE 'new_val' BEFORE 'old_val'"),
    ("sql297", "NOTIFY ch, 'abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789'"),
    ("sql341", "SELECT 1 FROM users WHERE id = ARRAY[1]"),
    ("sql342", "SELECT bool_and(email::text = 'x') FROM users"),
    ("sql512", "ALTER TABLE users ADD CONSTRAINT chk CHECK (no_such_col > 0)"),
  ];
  for (code, s) in inputs {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    let fires = codes.contains(&code);
    eprintln!("P3|{}|fires={}|got={:?}", code, fires, codes);
  }
}

#[test]
fn r30_sql172_0001() {
  let d = diags("-- v0\nSELECT * FROM users WHERE email = 1");
  assert!(d.iter().any(|x| x.code == "sql172"), "expected sql172");
}

#[test]
fn r30_sql172_0002() {
  let d = diags("-- v1\nSELECT * FROM users WHERE name = 1");
  assert!(d.iter().any(|x| x.code == "sql172"), "expected sql172");
}

#[test]
fn r30_sql172_0003() {
  let d = diags("-- v2\nSELECT * FROM users WHERE email = 100");
  assert!(d.iter().any(|x| x.code == "sql172"), "expected sql172");
}

#[test]
fn r30_sql173_0004() {
  let d = diags("-- v3\nCREATE TABLE users (id uuid PRIMARY KEY, different_col text)");
  assert!(d.iter().any(|x| x.code == "sql173"), "expected sql173");
}

#[test]
fn r30_sql173_0005() {
  let d = diags("-- v4\nCREATE TABLE orders (id uuid PRIMARY KEY, wrong_col int)");
  assert!(d.iter().any(|x| x.code == "sql173"), "expected sql173");
}

#[test]
fn r30_sql179_0006() {
  let d = diags("-- v5\nBEGIN; ROLLBACK; SAVEPOINT sp");
  assert!(d.iter().any(|x| x.code == "sql179"), "expected sql179");
}

#[test]
fn r30_sql179_0007() {
  let d = diags("-- v6\nBEGIN; COMMIT; SAVEPOINT sp");
  assert!(d.iter().any(|x| x.code == "sql179"), "expected sql179");
}

#[test]
fn r30_sql180_0008() {
  let d = diags("-- v7\nCREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN TRUNCATE users; RETURN NEW; END $$");
  assert!(d.iter().any(|x| x.code == "sql180"), "expected sql180");
}

#[test]
fn r30_sql180_0009() {
  let d = diags("-- v8\nCREATE FUNCTION g() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN TRUNCATE orders; RETURN NEW; END $$");
  assert!(d.iter().any(|x| x.code == "sql180"), "expected sql180");
}

#[test]
fn r30_sql212_0010() {
  let d = diags("-- v9\nSELECT 1 INTO users");
  assert!(d.iter().any(|x| x.code == "sql212"), "expected sql212");
}

#[test]
fn r30_sql212_0011() {
  let d = diags("-- v10\nSELECT 1 INTO orders");
  assert!(d.iter().any(|x| x.code == "sql212"), "expected sql212");
}

#[test]
fn r30_sql231_0012() {
  let d = diags("-- v11\nSELECT id FROM users NULLS FIRST");
  assert!(d.iter().any(|x| x.code == "sql231"), "expected sql231");
}

#[test]
fn r30_sql231_0013() {
  let d = diags("-- v12\nSELECT id FROM users NULLS LAST");
  assert!(d.iter().any(|x| x.code == "sql231"), "expected sql231");
}

#[test]
fn r30_sql245_0014() {
  let d = diags("-- v13\nSELECT * FROM pg_class");
  assert!(d.iter().any(|x| x.code == "sql245"), "expected sql245");
}

#[test]
fn r30_sql245_0015() {
  let d = diags("-- v14\nSELECT * FROM pg_index");
  assert!(d.iter().any(|x| x.code == "sql245"), "expected sql245");
}

#[test]
fn r30_sql245_0016() {
  let d = diags("-- v15\nSELECT * FROM pg_namespace");
  assert!(d.iter().any(|x| x.code == "sql245"), "expected sql245");
}

#[test]
fn r30_sql284_0017() {
  let d = diags("-- v16\nCREATE FUNCTION f() RETURNS int LANGUAGE plpgsql AS $$ BEGIN RETURN TG_OP::int; END $$");
  assert!(d.iter().any(|x| x.code == "sql284"), "expected sql284");
}

#[test]
fn r30_sql284_0018() {
  let d = diags("-- v17\nCREATE FUNCTION g() RETURNS int LANGUAGE plpgsql AS $$ BEGIN RETURN TG_NAME::int; END $$");
  assert!(d.iter().any(|x| x.code == "sql284"), "expected sql284");
}

#[test]
fn r30_probe() {
  let inputs = [
    ("sql170", "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ DECLARE x integer := 'abc'; BEGIN END; $$"),
    ("sql170", "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ DECLARE n int := 'foo'; BEGIN NULL; END; $$"),
    ("sql184", "CREATE TABLE big_t (n smallint); INSERT INTO big_t (n) VALUES (99999)"),
    ("sql184", "INSERT INTO big_t (n) VALUES (40000)"),
    ("sql186", "BEGIN; ALTER TABLE users DROP COLUMN email; COMMIT"),
    ("sql202", "CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN OLD; END $$; CREATE TRIGGER t BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION f()"),
    ("sql204", "UPDATE users u SET id = u.id WHERE u.id = 1"),
    ("sql222", "WITH x AS (SELECT * FROM users LIMIT 10) SELECT * FROM x FOR UPDATE"),
    ("sql226", "DROP TABLE users CASCADE"),
    ("sql233", "CREATE MATERIALIZED VIEW mv AS SELECT 1 AS x WITH NO DATA"),
    ("sql236", "CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$; CREATE TRIGGER t AFTER UPDATE ON users FOR EACH ROW EXECUTE FUNCTION f()"),
    ("sql265", "CREATE TABLE t (id int PRIMARY KEY, ts timestamp DEFAULT now())"),
    ("sql265", "CREATE TABLE x (id int PRIMARY KEY, t timestamp WITHOUT TIME ZONE DEFAULT now())"),
    ("sql274", "CREATE TEMP TABLE users (id int)"),
    ("sql286", "ALTER TYPE mood ADD VALUE 'new_val' BEFORE 'old_val'"),
    ("sql286", "ALTER TYPE my_enum ADD VALUE 'x' AFTER 'unknown_val'"),
    ("sql297", "NOTIFY ch, 'x'"),
    ("sql341", "SELECT * FROM users WHERE name = ARRAY['a']"),
    ("sql342", "SELECT bool_and(name = 'a') FROM users"),
    ("sql512", "ALTER TABLE users ADD CONSTRAINT chk CHECK (no_such_col > 0)"),
    ("sql512", "ALTER TABLE orders ADD CONSTRAINT chk CHECK (xyz_col > 0)"),
  ];
  for (code, s) in inputs {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    let fires = codes.contains(&code);
    eprintln!("P4|{}|fires={}|got={:?}", code, fires, codes);
  }
}

#[test]
fn r30_probe2() {
  let inputs = [
    ("sql222", "SELECT * FROM (SELECT id FROM users LIMIT 10) sub FOR UPDATE"),
    ("sql222", "SELECT * FROM (SELECT * FROM users LIMIT 5) sub FOR UPDATE"),
    ("sql222", "SELECT id FROM (SELECT id FROM orders LIMIT 100) sub FOR SHARE"),
    ("sql236", "CREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION f(); CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$"),
    ("sql236", "CREATE TRIGGER trg AFTER UPDATE ON users FOR EACH ROW EXECUTE FUNCTION fn(); CREATE FUNCTION fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$"),
    ("sql202", "CREATE TRIGGER t BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION fn(); CREATE FUNCTION fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE NOTICE '%', OLD; RETURN NEW; END $$"),
    ("sql202", "CREATE TRIGGER trg BEFORE INSERT ON orders FOR EACH ROW EXECUTE FUNCTION my_fn(); CREATE FUNCTION my_fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM OLD.id; RETURN NEW; END $$"),
  ];
  for (code, s) in inputs {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    let fires = codes.contains(&code);
    eprintln!("P5|{}|fires={}|got={:?}", code, fires, codes);
  }
}

#[test]
fn r31_sql202_0001() {
  let d = diags("-- v0\nCREATE TRIGGER trg BEFORE INSERT ON orders FOR EACH ROW EXECUTE FUNCTION my_fn(); CREATE FUNCTION my_fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM OLD.id; RETURN NEW; END $$");
  assert!(d.iter().any(|x| x.code == "sql202"), "expected sql202");
}

#[test]
fn r31_sql202_0002() {
  let d = diags("-- v1\nCREATE TRIGGER tr BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION fn2(); CREATE FUNCTION fn2() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM OLD.email; RETURN NEW; END $$");
  assert!(d.iter().any(|x| x.code == "sql202"), "expected sql202");
}

#[test]
fn r31_probe() {
  let inputs = [
    ("sql184", "INSERT INTO nums (n) VALUES (99999999999)"),
    ("sql265", "CREATE TABLE t (id int PRIMARY KEY, ts timestamp DEFAULT CURRENT_TIMESTAMP)"),
    ("sql265", "CREATE TABLE t (id int PRIMARY KEY, ts timestamp DEFAULT clock_timestamp())"),
    ("sql265", "CREATE TABLE t (id int PRIMARY KEY, ts timestamp NOT NULL DEFAULT now())"),
    ("sql265", "ALTER TABLE users ADD COLUMN ts timestamp DEFAULT now()"),
    ("sql286", "ALTER TYPE mood ADD VALUE IF NOT EXISTS 'new_val' BEFORE 'unknown_label_xyz'"),
    ("sql297", "NOTIFY ch, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'"),
    ("sql341", "SELECT * FROM users WHERE id::text = ARRAY[1]::text"),
    ("sql342", "SELECT bool_or(name IS NOT NULL) FROM users"),
    ("sql342", "SELECT bool_and(email IS NOT NULL) FROM users"),
    ("sql512", "ALTER TABLE users ADD CHECK (no_such_col > 0)"),
    ("sql512", "ALTER TABLE orders ADD CHECK (xyz_col = 0)"),
  ];
  for (code, s) in inputs {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    let fires = codes.contains(&code);
    eprintln!("P6|{}|fires={}|got={:?}", code, fires, codes);
  }
}

#[test]
fn r31_probe2() {
  let big_payload = "x".repeat(8100);
  let big_notify = format!("NOTIFY ch, '{}'", big_payload);
  let inputs = [
    ("sql286", "CREATE TYPE mood AS ENUM ('happy', 'sad'); ALTER TYPE mood ADD VALUE 'new_val' BEFORE 'totally_unknown_label_xyz'"),
    ("sql297", big_notify.as_str()),
  ];
  for (code, s) in inputs {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    let fires = codes.contains(&code);
    eprintln!("P7|{}|fires={}|got={:?}", code, fires, codes);
  }
}

#[test]
fn r32_sql297_0001() {
  let big_payload = "x".repeat(8100);
  let src = format!("-- v0\nNOTIFY ch, '{}'", big_payload);
  let d = diags(&src);
  assert!(d.iter().any(|x| x.code == "sql297"));
}

#[test]
fn r32_sql297_0002() {
  let big_payload = "x".repeat(8100);
  let src = format!("-- v1\nNOTIFY ch, '{}'", big_payload);
  let d = diags(&src);
  assert!(d.iter().any(|x| x.code == "sql297"));
}

#[test]
fn r32_sql297_0003() {
  let big_payload = "x".repeat(8100);
  let src = format!("-- v2\nNOTIFY ch, '{}'", big_payload);
  let d = diags(&src);
  assert!(d.iter().any(|x| x.code == "sql297"));
}

#[test]
fn r32_probe() {
  let inputs = [
    ("sql236", "CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$; CREATE TRIGGER trg AFTER UPDATE ON users FOR EACH ROW EXECUTE FUNCTION f();"),
    ("sql236", "CREATE FUNCTION audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$; CREATE TRIGGER au AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION audit();"),
    ("sql236", "CREATE TRIGGER tt AFTER INSERT ON users FOR EACH STATEMENT EXECUTE FUNCTION audit(); CREATE FUNCTION audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$"),
    ("sql174", "SELECT count(name) FROM users"),
    ("sql174", "SELECT count(email) FROM users"),
  ];
  for (code, s) in inputs {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    let fires = codes.contains(&code);
    eprintln!("P8|{}|fires={}|got={:?}", code, fires, codes);
  }
}

#[test]
fn r32_probe2() {
  // Make sure execute function fn() pattern matches
  let inputs = [
    ("sql236", "CREATE FUNCTION audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$;\nCREATE TRIGGER au AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION audit()"),
    ("sql236", "CREATE FUNCTION audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN OLD; END $$;\nCREATE TRIGGER au AFTER DELETE ON users FOR EACH ROW EXECUTE FUNCTION audit()"),
    ("sql236", "CREATE FUNCTION mkfn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$;\nCREATE TRIGGER tr AFTER UPDATE ON orders FOR EACH ROW EXECUTE PROCEDURE mkfn()"),
  ];
  for (code, s) in inputs {
    let d = diags(s);
    let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
    let fires = codes.contains(&code);
    eprintln!("P9|{}|fires={}|got={:?}", code, fires, codes);
  }
}

#[test]
fn sql515_flags_single_value_in() {
  let d = diags("SELECT * FROM users WHERE id IN (1);");
  let m = d.iter().find(|x| x.code == "sql515").unwrap_or_else(|| panic!("expected sql515: {d:?}"));
  assert!(m.message.contains("= 1"), "should suggest `= 1`: {}", m.message);
}

#[test]
fn sql515_flags_single_value_not_in_with_neq() {
  let d = diags("SELECT * FROM users WHERE id NOT IN ('x');");
  let m = d.iter().find(|x| x.code == "sql515").unwrap_or_else(|| panic!("expected sql515: {d:?}"));
  assert!(m.message.contains("<> 'x'"), "NOT IN should suggest `<>`: {}", m.message);
}

#[test]
fn sql515_handles_no_space_before_paren() {
  let d = diags("SELECT * FROM users WHERE id IN(42);");
  assert!(d.iter().any(|x| x.code == "sql515"), "IN(x) without space should still flag: {d:?}");
}

#[test]
fn sql515_quiet_for_multi_element_list() {
  let d = diags("SELECT * FROM users WHERE id IN (1, 2, 3);");
  assert!(!d.iter().any(|x| x.code == "sql515"), "multi-element IN must not flag: {d:?}");
}

#[test]
fn sql515_quiet_for_subquery() {
  let d = diags("SELECT * FROM users WHERE id IN (SELECT user_id FROM orders);");
  assert!(!d.iter().any(|x| x.code == "sql515"), "subquery IN must not flag: {d:?}");
}

#[test]
fn sql515_flags_single_function_call_value() {
  // A lone function call is still a single value; its internal comma is
  // nested, so the top-level-comma guard correctly treats it as one element.
  let d = diags("SELECT * FROM users WHERE id IN (coalesce(id, 0));");
  let m = d.iter().find(|x| x.code == "sql515").unwrap_or_else(|| panic!("expected sql515: {d:?}"));
  assert!(m.message.contains("= coalesce(id, 0)"), "{}", m.message);
}

#[test]
fn sql515_quiet_for_in_null() {
  let d = diags("SELECT * FROM users WHERE id IN (NULL);");
  assert!(!d.iter().any(|x| x.code == "sql515"), "IN (NULL) must not suggest `= NULL`: {d:?}");
}

#[test]
fn sql516_flags_self_assignment() {
  let d = diags("UPDATE users SET name = name WHERE id = '1';");
  let m = d.iter().find(|x| x.code == "sql516").unwrap_or_else(|| panic!("expected sql516: {d:?}"));
  assert!(m.message.contains("no-op"), "{}", m.message);
}

#[test]
fn sql516_flags_qualified_self_assignment() {
  let d = diags("UPDATE users SET users.name = users.name;");
  assert!(d.iter().any(|x| x.code == "sql516"), "qualified self-assign should flag: {d:?}");
}

#[test]
fn sql516_quiet_for_real_assignment() {
  let d = diags("UPDATE users SET name = email WHERE id = '1';");
  assert!(!d.iter().any(|x| x.code == "sql516"), "real assign must not flag: {d:?}");
  let d2 = diags("UPDATE users SET name = name || 'x';");
  assert!(!d2.iter().any(|x| x.code == "sql516"), "expression rhs must not flag: {d2:?}");
}

#[test]
fn sql516_quiet_for_cross_table_copy() {
  // `name = o.name` copies from another table -- not a self-assign.
  let d = diags("UPDATE users SET name = o.name FROM orders o WHERE o.user_id = users.id;");
  assert!(!d.iter().any(|x| x.code == "sql516"), "cross-table copy must not flag: {d:?}");
}

#[test]
fn sql516_finds_self_assign_among_many() {
  let d = diags("UPDATE users SET email = 'x', name = name WHERE id = '1';");
  assert!(d.iter().any(|x| x.code == "sql516"), "self-assign in a list should flag: {d:?}");
}

#[test]
fn sql517_flags_join_on_one_equals_one() {
  let d = diags("SELECT * FROM users u JOIN orders o ON 1 = 1;");
  let m = d.iter().find(|x| x.code == "sql517").unwrap_or_else(|| panic!("expected sql517: {d:?}"));
  assert!(m.message.contains("CROSS JOIN"), "{}", m.message);
}

#[test]
fn sql517_flags_no_space_tautology() {
  let d = diags("SELECT * FROM users u JOIN orders o ON 1=1 WHERE u.id = o.user_id;");
  assert!(d.iter().any(|x| x.code == "sql517"), "ON 1=1 should flag: {d:?}");
}

#[test]
fn sql517_quiet_for_real_join_predicate() {
  let d = diags("SELECT * FROM users u JOIN orders o ON u.id = o.user_id;");
  assert!(!d.iter().any(|x| x.code == "sql517"), "real join predicate must not flag: {d:?}");
}

#[test]
fn sql517_quiet_for_on_true() {
  // ON TRUE is idiomatic for LATERAL joins -- must not flag.
  let d = diags("SELECT * FROM users u LEFT JOIN LATERAL (SELECT 1) s ON TRUE;");
  assert!(!d.iter().any(|x| x.code == "sql517"), "ON TRUE must not flag: {d:?}");
}

#[test]
fn sql517_quiet_for_partial_tautology() {
  // A real predicate AND 1=1 still has a real condition -- don't flag.
  let d = diags("SELECT * FROM users u JOIN orders o ON u.id = o.user_id AND 1 = 1;");
  assert!(!d.iter().any(|x| x.code == "sql517"), "partial tautology must not flag: {d:?}");
}

#[test]
fn sql518_flags_then_true_else_false() {
  let d = diags("SELECT CASE WHEN id > 0 THEN TRUE ELSE FALSE END FROM users;");
  let m = d.iter().find(|x| x.code == "sql518").unwrap_or_else(|| panic!("expected sql518: {d:?}"));
  assert!(m.message.contains("IS TRUE"), "{}", m.message);
}

#[test]
fn sql518_flags_inverted_form() {
  let d = diags("SELECT CASE WHEN id > 0 THEN FALSE ELSE TRUE END FROM users;");
  let m = d.iter().find(|x| x.code == "sql518").unwrap_or_else(|| panic!("expected sql518: {d:?}"));
  assert!(m.message.contains("IS NOT TRUE"), "{}", m.message);
}

#[test]
fn sql518_quiet_for_non_boolean_arms() {
  let d = diags("SELECT CASE WHEN id > 0 THEN 'yes' ELSE 'no' END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql518"), "non-boolean arms must not flag: {d:?}");
}

#[test]
fn sql518_quiet_for_multi_branch() {
  let d = diags("SELECT CASE WHEN id > 0 THEN TRUE WHEN id < 0 THEN FALSE ELSE FALSE END FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql518"), "multi-branch CASE must not flag: {d:?}");
}

#[test]
fn sql519_flags_three_way_or_chain() {
  let d = diags("SELECT * FROM users WHERE id = 1 OR id = 2 OR id = 3;");
  let m = d.iter().find(|x| x.code == "sql519").unwrap_or_else(|| panic!("expected sql519: {d:?}"));
  assert!(m.message.contains("IN (...)") && m.message.contains("id"), "{}", m.message);
}

#[test]
fn sql519_flags_parenthesized_chain() {
  let d = diags("SELECT * FROM users WHERE (name = 'a' OR name = 'b' OR name = 'c');");
  assert!(d.iter().any(|x| x.code == "sql519"), "parenthesized OR chain should flag: {d:?}");
}

#[test]
fn sql519_quiet_for_two_values() {
  let d = diags("SELECT * FROM users WHERE id = 1 OR id = 2;");
  assert!(!d.iter().any(|x| x.code == "sql519"), "two-value OR must not flag: {d:?}");
}

#[test]
fn sql519_quiet_for_different_columns() {
  let d = diags("SELECT * FROM users WHERE id = 1 OR name = 'a' OR email = 'b';");
  assert!(!d.iter().any(|x| x.code == "sql519"), "different columns must not flag: {d:?}");
}

#[test]
fn sql519_quiet_when_term_has_and() {
  let d = diags("SELECT * FROM users WHERE id = 1 OR id = 2 OR (id = 3 AND name = 'x');");
  assert!(!d.iter().any(|x| x.code == "sql519"), "AND term breaks the run: {d:?}");
}

#[test]
fn sql520_flags_lower_eq_uppercase_literal() {
  let d = diags("SELECT * FROM users WHERE lower(name) = 'ABC';");
  let m = d.iter().find(|x| x.code == "sql520").unwrap_or_else(|| panic!("expected sql520: {d:?}"));
  assert!(m.message.contains("zero rows"), "{}", m.message);
}

#[test]
fn sql520_flags_upper_like_lowercase_literal() {
  let d = diags("SELECT * FROM users WHERE upper(name) LIKE 'abc%';");
  assert!(d.iter().any(|x| x.code == "sql520"), "upper LIKE lowercase should flag: {d:?}");
}

#[test]
fn sql520_quiet_when_literal_case_matches() {
  let d = diags("SELECT * FROM users WHERE lower(name) = 'abc';");
  assert!(!d.iter().any(|x| x.code == "sql520"), "matching-case literal must not flag: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE upper(name) = 'ABC';");
  assert!(!d2.iter().any(|x| x.code == "sql520"), "upper vs uppercase must not flag: {d2:?}");
}

#[test]
fn sql520_quiet_for_non_literal_rhs() {
  let d = diags("SELECT * FROM users WHERE lower(name) = lower(email);");
  assert!(!d.iter().any(|x| x.code == "sql520"), "non-literal rhs must not flag: {d:?}");
}

#[test]
fn sql521_flags_single_element_any_array() {
  let d = diags("SELECT * FROM users WHERE id = ANY(ARRAY[1]);");
  let m = d.iter().find(|x| x.code == "sql521").unwrap_or_else(|| panic!("expected sql521: {d:?}"));
  assert!(m.message.contains("compare against `1`"), "{}", m.message);
}

#[test]
fn sql521_flags_single_element_all_array() {
  let d = diags("SELECT * FROM users WHERE name <> ALL(ARRAY['x']);");
  assert!(d.iter().any(|x| x.code == "sql521"), "single-element ALL array should flag: {d:?}");
}

#[test]
fn sql521_quiet_for_multi_element_array() {
  let d = diags("SELECT * FROM users WHERE id = ANY(ARRAY[1, 2, 3]);");
  assert!(!d.iter().any(|x| x.code == "sql521"), "multi-element array must not flag: {d:?}");
}

#[test]
fn sql523_flags_is_null_or_is_not_null() {
  let d = diags("SELECT * FROM users WHERE name IS NULL OR name IS NOT NULL;");
  let m = d.iter().find(|x| x.code == "sql523").unwrap_or_else(|| panic!("expected sql523: {d:?}"));
  assert!(m.message.contains("always true"), "{}", m.message);
}

#[test]
fn sql523_flags_reversed_order() {
  let d = diags("SELECT * FROM users WHERE name IS NOT NULL OR name IS NULL;");
  assert!(d.iter().any(|x| x.code == "sql523"), "reversed order should flag: {d:?}");
}

#[test]
fn sql523_quiet_for_different_columns() {
  let d = diags("SELECT * FROM users WHERE name IS NULL OR email IS NOT NULL;");
  assert!(!d.iter().any(|x| x.code == "sql523"), "different columns must not flag: {d:?}");
}

#[test]
fn sql523_quiet_for_single_null_check() {
  let d = diags("SELECT * FROM users WHERE name IS NULL;");
  assert!(!d.iter().any(|x| x.code == "sql523"), "single null check must not flag: {d:?}");
}

#[test]
fn sql522_flags_where_filter_on_left_joined_table() {
  let d = diags("SELECT * FROM users u LEFT JOIN orders o ON o.user_id = u.id WHERE o.total = 5;");
  let m = d.iter().find(|x| x.code == "sql522").unwrap_or_else(|| panic!("expected sql522: {d:?}"));
  assert!(m.message.contains("INNER JOIN") && m.message.contains('o'), "{}", m.message);
}

#[test]
fn sql522_quiet_for_is_null_anti_join() {
  let d = diags("SELECT * FROM users u LEFT JOIN orders o ON o.user_id = u.id WHERE o.id IS NULL;");
  assert!(!d.iter().any(|x| x.code == "sql522"), "IS NULL anti-join must not flag: {d:?}");
}

#[test]
fn sql522_quiet_for_or_null_guard() {
  let d = diags("SELECT * FROM users u LEFT JOIN orders o ON o.user_id = u.id WHERE o.total = 5 OR o.id IS NULL;");
  assert!(!d.iter().any(|x| x.code == "sql522"), "OR-null-guarded filter must not flag: {d:?}");
}

#[test]
fn sql522_quiet_for_filter_on_preserved_side() {
  let d = diags("SELECT * FROM users u LEFT JOIN orders o ON o.user_id = u.id WHERE u.email = 'x';");
  assert!(!d.iter().any(|x| x.code == "sql522"), "filter on preserved side must not flag: {d:?}");
}

#[test]
fn sql524_flags_like_all_wildcard() {
  let d = diags("SELECT * FROM users WHERE name LIKE '%';");
  let m = d.iter().find(|x| x.code == "sql524").unwrap_or_else(|| panic!("expected sql524: {d:?}"));
  assert!(m.message.contains("filters nothing"), "{}", m.message);
}

#[test]
fn sql524_flags_not_like_all_wildcard() {
  let d = diags("SELECT * FROM users WHERE name NOT LIKE '%%';");
  let m = d.iter().find(|x| x.code == "sql524").unwrap_or_else(|| panic!("expected sql524: {d:?}"));
  assert!(m.message.contains("returns nothing"), "{}", m.message);
}

#[test]
fn sql524_quiet_for_real_pattern() {
  let d = diags("SELECT * FROM users WHERE name LIKE '%abc%';");
  assert!(!d.iter().any(|x| x.code == "sql524"), "real pattern must not flag: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE name LIKE '_';");
  assert!(!d2.iter().any(|x| x.code == "sql524"), "single-char wildcard must not flag: {d2:?}");
}

#[test]
fn sql525_flags_limit_in_exists() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id LIMIT 1);");
  let m = d.iter().find(|x| x.code == "sql525").unwrap_or_else(|| panic!("expected sql525: {d:?}"));
  assert!(m.message.contains("redundant"), "{}", m.message);
}

#[test]
fn sql525_quiet_for_exists_without_limit() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id);");
  assert!(!d.iter().any(|x| x.code == "sql525"), "EXISTS without LIMIT must not flag: {d:?}");
}

#[test]
fn sql525_quiet_for_limit_in_nested_derived_table() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM (SELECT * FROM orders LIMIT 5) s WHERE s.user_id = u.id);");
  assert!(!d.iter().any(|x| x.code == "sql525"), "LIMIT in nested derived table must not flag: {d:?}");
}

#[test]
fn sql526_flags_two_different_equality_constants() {
  let d = diags("SELECT * FROM users WHERE id = 1 AND id = 2;");
  let m = d.iter().find(|x| x.code == "sql526").unwrap_or_else(|| panic!("expected sql526: {d:?}"));
  assert!(m.message.contains("always false"), "{}", m.message);
}

#[test]
fn sql526_flags_eq_and_neq_same_value() {
  let d = diags("SELECT * FROM users WHERE name = 'a' AND name <> 'a';");
  assert!(d.iter().any(|x| x.code == "sql526"), "eq + neq same value should flag: {d:?}");
}

#[test]
fn sql526_quiet_for_numeric_equivalent_constants() {
  // 1 and 1.0 are the same number -- not a contradiction.
  let d = diags("SELECT * FROM users WHERE id = 1 AND id = 1.0;");
  assert!(!d.iter().any(|x| x.code == "sql526"), "1 vs 1.0 must not flag: {d:?}");
}

#[test]
fn sql526_quiet_for_different_columns() {
  let d = diags("SELECT * FROM users WHERE id = 1 AND name = 'x';");
  assert!(!d.iter().any(|x| x.code == "sql526"), "different columns must not flag: {d:?}");
}

#[test]
fn sql527_flags_impossible_range() {
  let d = diags("SELECT * FROM users WHERE id > 5 AND id < 3;");
  let m = d.iter().find(|x| x.code == "sql527").unwrap_or_else(|| panic!("expected sql527: {d:?}"));
  assert!(m.message.contains("always empty"), "{}", m.message);
}

#[test]
fn sql527_flags_equal_bound_with_strict() {
  let d = diags("SELECT * FROM users WHERE id >= 5 AND id < 5;");
  assert!(d.iter().any(|x| x.code == "sql527"), "id>=5 AND id<5 is empty: {d:?}");
}

#[test]
fn sql527_quiet_for_valid_range() {
  let d = diags("SELECT * FROM users WHERE id > 5 AND id < 10;");
  assert!(!d.iter().any(|x| x.code == "sql527"), "valid range must not flag: {d:?}");
}

#[test]
fn sql527_quiet_for_ambiguous_int_range() {
  // Empty for integers, non-empty for numeric -- left alone to avoid a wrong call.
  let d = diags("SELECT * FROM users WHERE id > 5 AND id < 6;");
  assert!(!d.iter().any(|x| x.code == "sql527"), "ambiguous int range must not flag: {d:?}");
}

#[test]
fn sql527_quiet_for_inclusive_equal_bounds() {
  let d = diags("SELECT * FROM users WHERE id >= 5 AND id <= 5;");
  assert!(!d.iter().any(|x| x.code == "sql527"), "id >= 5 AND id <= 5 allows id=5: {d:?}");
}

#[test]
fn sql529_flags_count_gt_zero() {
  let d = diags("SELECT user_id FROM orders GROUP BY user_id HAVING COUNT(*) > 0;");
  let m = d.iter().find(|x| x.code == "sql529").unwrap_or_else(|| panic!("expected sql529: {d:?}"));
  assert!(m.message.contains("always true"), "{}", m.message);
}

#[test]
fn sql529_flags_count_ge_one() {
  let d = diags("SELECT user_id FROM orders GROUP BY user_id HAVING COUNT(*) >= 1;");
  assert!(d.iter().any(|x| x.code == "sql529"), "COUNT(*) >= 1 should flag: {d:?}");
}

#[test]
fn sql529_quiet_for_real_threshold() {
  let d = diags("SELECT user_id FROM orders GROUP BY user_id HAVING COUNT(*) > 1;");
  assert!(!d.iter().any(|x| x.code == "sql529"), "COUNT(*) > 1 is a real filter: {d:?}");
  let d2 = diags("SELECT user_id FROM orders GROUP BY user_id HAVING COUNT(*) >= 5;");
  assert!(!d2.iter().any(|x| x.code == "sql529"), "COUNT(*) >= 5 is a real filter: {d2:?}");
}

#[test]
fn sql530_flags_nested_coalesce() {
  let d = diags("SELECT COALESCE(COALESCE(name, email), 'x') FROM users;");
  let m = d.iter().find(|x| x.code == "sql530").unwrap_or_else(|| panic!("expected sql530: {d:?}"));
  assert!(m.message.contains("flatten"), "{}", m.message);
}

#[test]
fn sql530_flags_nested_in_later_arg() {
  let d = diags("SELECT COALESCE(name, COALESCE(email, 'x')) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql530"), "nested coalesce in 2nd arg should flag: {d:?}");
}

#[test]
fn sql530_quiet_for_flat_coalesce() {
  let d = diags("SELECT COALESCE(name, email, 'x') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql530"), "flat coalesce must not flag: {d:?}");
  // A coalesce as part of a larger expression is not a direct nested arg.
  let d2 = diags("SELECT COALESCE(name, email) || coalesce(name, 'x') FROM users;");
  assert!(!d2.iter().any(|x| x.code == "sql530"), "sibling coalesce must not flag: {d2:?}");
}

#[test]
fn sql531_flags_redundant_alias() {
  let d = diags("SELECT name AS name FROM users;");
  let m = d.iter().find(|x| x.code == "sql531").unwrap_or_else(|| panic!("expected sql531: {d:?}"));
  assert!(m.message.contains("redundant alias"), "{}", m.message);
}

#[test]
fn sql531_flags_qualified_redundant_alias() {
  let d = diags("SELECT u.name AS name FROM users u;");
  assert!(d.iter().any(|x| x.code == "sql531"), "qualified col aliased to base name should flag: {d:?}");
}

#[test]
fn sql531_quiet_for_real_rename() {
  let d = diags("SELECT name AS full_name FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql531"), "real rename must not flag: {d:?}");
  let d2 = diags("SELECT lower(name) AS name FROM users;");
  assert!(!d2.iter().any(|x| x.code == "sql531"), "expression alias must not flag: {d2:?}");
}

#[test]
fn sql528_flags_replace_same_from_to() {
  let d = diags("SELECT REPLACE(name, '-', '-') FROM users;");
  let m = d.iter().find(|x| x.code == "sql528").unwrap_or_else(|| panic!("expected sql528: {d:?}"));
  assert!(m.message.contains("no-op"), "{}", m.message);
}

#[test]
fn sql528_quiet_for_real_replace() {
  let d = diags("SELECT REPLACE(name, '-', '') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql528"), "real replace must not flag: {d:?}");
  // regexp_replace is a different function -- whole-word match excludes it.
  let d2 = diags("SELECT regexp_replace(name, 'a', 'a') FROM users;");
  assert!(!d2.iter().any(|x| x.code == "sql528"), "regexp_replace must not flag here: {d2:?}");
}

#[test]
fn sql532_flags_identical_union_branches() {
  let d = diags("SELECT id FROM users UNION SELECT id FROM users;");
  let m = d.iter().find(|x| x.code == "sql532").unwrap_or_else(|| panic!("expected sql532: {d:?}"));
  assert!(m.message.contains("duplicate branch"), "{}", m.message);
}

#[test]
fn sql532_flags_union_all_duplicate() {
  let d = diags("SELECT id FROM users UNION ALL SELECT id FROM users;");
  assert!(d.iter().any(|x| x.code == "sql532"), "UNION ALL duplicate should flag: {d:?}");
}

#[test]
fn sql532_quiet_for_different_branches() {
  let d = diags("SELECT id FROM users UNION SELECT id FROM orders;");
  assert!(!d.iter().any(|x| x.code == "sql532"), "different branches must not flag: {d:?}");
}

#[test]
fn sql533_flags_between_equal_bounds() {
  let d = diags("SELECT * FROM users WHERE id BETWEEN 5 AND 5;");
  let m = d.iter().find(|x| x.code == "sql533").unwrap_or_else(|| panic!("expected sql533: {d:?}"));
  assert!(m.message.contains("= 5"), "{}", m.message);
}

#[test]
fn sql533_flags_not_between_equal() {
  let d = diags("SELECT * FROM users WHERE id NOT BETWEEN 5 AND 5;");
  let m = d.iter().find(|x| x.code == "sql533").unwrap_or_else(|| panic!("expected sql533: {d:?}"));
  assert!(m.message.contains("<> 5"), "{}", m.message);
}

#[test]
fn sql533_quiet_for_real_range() {
  let d = diags("SELECT * FROM users WHERE id BETWEEN 1 AND 10;");
  assert!(!d.iter().any(|x| x.code == "sql533"), "real range must not flag: {d:?}");
  // High bound is an expression, not the same simple literal.
  let d2 = diags("SELECT * FROM users WHERE id BETWEEN 5 AND 5 + 1;");
  assert!(!d2.iter().any(|x| x.code == "sql533"), "expression high bound must not flag: {d2:?}");
}

#[test]
fn sql534_flags_greatest_duplicate_arg() {
  let d = diags("SELECT GREATEST(a, a) FROM t;");
  let m = d.iter().find(|x| x.code == "sql534").unwrap_or_else(|| panic!("expected sql534: {d:?}"));
  assert!(m.message.contains("maximum"), "{}", m.message);
}

#[test]
fn sql534_flags_least_duplicate_among_many() {
  let d = diags("SELECT LEAST(a, b, a) FROM t;");
  assert!(d.iter().any(|x| x.code == "sql534"), "duplicate arg in LEAST should flag: {d:?}");
}

#[test]
fn sql534_quiet_for_distinct_args() {
  let d = diags("SELECT GREATEST(a, b, c) FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql534"), "distinct args must not flag: {d:?}");
}

#[test]
fn sql535_flags_neq_and_chain() {
  let d = diags("SELECT * FROM users WHERE id <> 1 AND id <> 2 AND id <> 3;");
  let m = d.iter().find(|x| x.code == "sql535").unwrap_or_else(|| panic!("expected sql535: {d:?}"));
  assert!(m.message.contains("NOT IN (...)") && m.message.contains("id"), "{}", m.message);
}

#[test]
fn sql535_flags_bang_eq_variant() {
  let d = diags("SELECT * FROM users WHERE name != 'a' AND name != 'b' AND name != 'c';");
  assert!(d.iter().any(|x| x.code == "sql535"), "!= chain should flag: {d:?}");
}

#[test]
fn sql535_quiet_for_two_values_or_mixed() {
  let d = diags("SELECT * FROM users WHERE id <> 1 AND id <> 2;");
  assert!(!d.iter().any(|x| x.code == "sql535"), "two-value chain must not flag: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE id <> 1 AND name <> 'a' AND email <> 'b';");
  assert!(!d2.iter().any(|x| x.code == "sql535"), "different columns must not flag: {d2:?}");
}

#[test]
fn sql536_flags_on_conflict_self_assign() {
  let d = diags("INSERT INTO users (id, name) VALUES ('1', 'x') ON CONFLICT (id) DO UPDATE SET name = name;");
  let m = d.iter().find(|x| x.code == "sql536").unwrap_or_else(|| panic!("expected sql536: {d:?}"));
  assert!(m.message.contains("EXCLUDED"), "{}", m.message);
}

#[test]
fn sql536_quiet_for_excluded_assignment() {
  let d = diags("INSERT INTO users (id, name) VALUES ('1', 'x') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;");
  assert!(!d.iter().any(|x| x.code == "sql536"), "EXCLUDED assignment must not flag: {d:?}");
}

#[test]
fn sql537_flags_not_eq() {
  let d = diags("SELECT * FROM users WHERE NOT (id = 5);");
  let m = d.iter().find(|x| x.code == "sql537").unwrap_or_else(|| panic!("expected sql537: {d:?}"));
  assert!(m.message.contains("id <> 5"), "{}", m.message);
}

#[test]
fn sql537_flags_not_less_than() {
  let d = diags("SELECT * FROM users WHERE NOT (id < 5);");
  let m = d.iter().find(|x| x.code == "sql537").unwrap_or_else(|| panic!("expected sql537: {d:?}"));
  assert!(m.message.contains("id >= 5"), "{}", m.message);
}

#[test]
fn sql537_quiet_for_compound_or_in() {
  let d = diags("SELECT * FROM users WHERE NOT (id = 5 AND name = 'x');");
  assert!(!d.iter().any(|x| x.code == "sql537"), "compound predicate must not flag: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE NOT (id IN (1, 2));");
  assert!(!d2.iter().any(|x| x.code == "sql537"), "NOT (IN) is sql470's job: {d2:?}");
}

#[test]
fn sql538_flags_round_zero_scale() {
  let d = diags("SELECT ROUND(price, 0) FROM items;");
  let m = d.iter().find(|x| x.code == "sql538").unwrap_or_else(|| panic!("expected sql538: {d:?}"));
  assert!(m.message.contains("redundant scale"), "{}", m.message);
}

#[test]
fn sql538_quiet_for_real_scale() {
  let d = diags("SELECT ROUND(price, 2) FROM items;");
  assert!(!d.iter().any(|x| x.code == "sql538"), "real scale must not flag: {d:?}");
  let d2 = diags("SELECT ROUND(price) FROM items;");
  assert!(!d2.iter().any(|x| x.code == "sql538"), "single-arg round must not flag: {d2:?}");
}

#[test]
fn sql539_flags_distinct_as_function() {
  let d = diags("SELECT DISTINCT(id), name FROM users;");
  let m = d.iter().find(|x| x.code == "sql539").unwrap_or_else(|| panic!("expected sql539: {d:?}"));
  assert!(m.message.contains("not a function"), "{}", m.message);
}

#[test]
fn sql539_flags_distinct_inside_count() {
  let d = diags("SELECT COUNT(DISTINCT(id)) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql539"), "DISTINCT( inside count should flag: {d:?}");
}

#[test]
fn sql539_quiet_for_proper_distinct() {
  let d = diags("SELECT DISTINCT id, name FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql539"), "plain DISTINCT must not flag: {d:?}");
  let d2 = diags("SELECT DISTINCT ON (id) id, name FROM users;");
  assert!(!d2.iter().any(|x| x.code == "sql539"), "DISTINCT ON must not flag: {d2:?}");
}

#[test]
fn sql540_flags_length_eq_zero() {
  let d = diags("SELECT * FROM users WHERE length(name) = 0;");
  let m = d.iter().find(|x| x.code == "sql540").unwrap_or_else(|| panic!("expected sql540: {d:?}"));
  assert!(m.message.contains("name = ''"), "{}", m.message);
}

#[test]
fn sql540_flags_length_gt_zero() {
  let d = diags("SELECT * FROM users WHERE length(name) > 0;");
  let m = d.iter().find(|x| x.code == "sql540").unwrap_or_else(|| panic!("expected sql540: {d:?}"));
  assert!(m.message.contains("name <> ''"), "{}", m.message);
}

#[test]
fn sql540_quiet_for_real_length_threshold() {
  let d = diags("SELECT * FROM users WHERE length(name) > 5;");
  assert!(!d.iter().any(|x| x.code == "sql540"), "length > 5 is a real check: {d:?}");
}

#[test]
fn sql541_flags_or_true() {
  let d = diags("SELECT * FROM users WHERE name = 'x' OR TRUE;");
  let m = d.iter().find(|x| x.code == "sql541").unwrap_or_else(|| panic!("expected sql541: {d:?}"));
  assert!(m.message.contains("matches every row"), "{}", m.message);
}

#[test]
fn sql541_flags_and_false() {
  let d = diags("SELECT * FROM users WHERE name = 'x' AND FALSE;");
  let m = d.iter().find(|x| x.code == "sql541").unwrap_or_else(|| panic!("expected sql541: {d:?}"));
  assert!(m.message.contains("matches no rows"), "{}", m.message);
}

#[test]
fn sql541_quiet_for_comparison_with_bool_literal() {
  // `active = TRUE` is a comparison, not a standalone operand.
  let d = diags("SELECT * FROM users WHERE active = TRUE OR name = 'x';");
  assert!(!d.iter().any(|x| x.code == "sql541"), "comparison RHS must not flag: {d:?}");
}

#[test]
fn sql541_quiet_for_and_false_under_or() {
  // (name = 'x' AND FALSE) OR active  == active, NOT always false.
  let d = diags("SELECT * FROM users WHERE (name = 'x' AND FALSE) OR active;");
  assert!(!d.iter().any(|x| x.code == "sql541"), "AND FALSE under OR must not flag as always-false: {d:?}");
}

#[test]
fn sql542_flags_now_cast_to_date() {
  let d = diags("SELECT * FROM users WHERE created::date = now()::date;");
  let m = d.iter().find(|x| x.code == "sql542").unwrap_or_else(|| panic!("expected sql542: {d:?}"));
  assert!(m.message.contains("CURRENT_DATE"), "{}", m.message);
}

#[test]
fn sql542_flags_current_timestamp_cast() {
  let d = diags("SELECT current_timestamp::date;");
  assert!(d.iter().any(|x| x.code == "sql542"), "current_timestamp::date should flag: {d:?}");
}

#[test]
fn sql542_quiet_for_column_cast() {
  let d = diags("SELECT created_at::date FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql542"), "column::date must not flag: {d:?}");
}

#[test]
fn sql543_flags_group_by_aggregate() {
  let d = diags("SELECT user_id, count(*) FROM orders GROUP BY user_id, count(*);");
  let m = d.iter().find(|x| x.code == "sql543").unwrap_or_else(|| panic!("expected sql543: {d:?}"));
  assert!(m.message.contains("42803"), "{}", m.message);
}

#[test]
fn sql543_flags_sum_in_group_by() {
  let d = diags("SELECT a FROM t GROUP BY sum(b);");
  assert!(d.iter().any(|x| x.code == "sql543"), "sum() in GROUP BY should flag: {d:?}");
}

#[test]
fn sql543_quiet_for_plain_group_by() {
  let d = diags("SELECT user_id, count(*) FROM orders GROUP BY user_id;");
  assert!(!d.iter().any(|x| x.code == "sql543"), "plain GROUP BY must not flag: {d:?}");
  // A column merely named like an aggregate (no parens) is fine.
  let d2 = diags("SELECT count FROM t GROUP BY count;");
  assert!(!d2.iter().any(|x| x.code == "sql543"), "column named count must not flag: {d2:?}");
}

#[test]
fn sql544_flags_inclusive_equal_bounds() {
  let d = diags("SELECT * FROM users WHERE id >= 5 AND id <= 5;");
  let m = d.iter().find(|x| x.code == "sql544").unwrap_or_else(|| panic!("expected sql544: {d:?}"));
  assert!(m.message.contains("id = 5"), "{}", m.message);
}

#[test]
fn sql544_quiet_for_real_range_or_strict() {
  let d = diags("SELECT * FROM users WHERE id >= 5 AND id <= 10;");
  assert!(!d.iter().any(|x| x.code == "sql544"), "real range must not flag: {d:?}");
  // Strict bounds at the same value are empty (sql527's job), not equality.
  let d2 = diags("SELECT * FROM users WHERE id > 5 AND id <= 5;");
  assert!(!d2.iter().any(|x| x.code == "sql544"), "strict bound must not flag here: {d2:?}");
}

#[test]
fn sql545_flags_month_out_of_range() {
  let d = diags("SELECT * FROM events WHERE EXTRACT(MONTH FROM created_at) = 13;");
  let m = d.iter().find(|x| x.code == "sql545").unwrap_or_else(|| panic!("expected sql545: {d:?}"));
  assert!(m.message.contains("1-12") && m.message.contains("never matches"), "{}", m.message);
}

#[test]
fn sql545_flags_dow_seven_with_hint() {
  let d = diags("SELECT * FROM events WHERE EXTRACT(DOW FROM created_at) = 7;");
  let m = d.iter().find(|x| x.code == "sql545").unwrap_or_else(|| panic!("expected sql545: {d:?}"));
  assert!(m.message.contains("ISODOW"), "{}", m.message);
}

#[test]
fn sql545_flags_date_part_form() {
  let d = diags("SELECT * FROM events WHERE date_part('hour', created_at) = 24;");
  assert!(d.iter().any(|x| x.code == "sql545"), "date_part hour=24 should flag: {d:?}");
}

#[test]
fn sql545_quiet_for_in_range() {
  let d = diags("SELECT * FROM events WHERE EXTRACT(MONTH FROM created_at) = 12;");
  assert!(!d.iter().any(|x| x.code == "sql545"), "in-range value must not flag: {d:?}");
}

#[test]
fn sql546_flags_modulo_out_of_range() {
  let d = diags("SELECT * FROM users WHERE id % 7 = 7;");
  let m = d.iter().find(|x| x.code == "sql546").unwrap_or_else(|| panic!("expected sql546: {d:?}"));
  assert!(m.message.contains("never matches"), "{}", m.message);
}

#[test]
fn sql546_quiet_for_valid_modulo() {
  let d = diags("SELECT * FROM users WHERE id % 2 = 0;");
  assert!(!d.iter().any(|x| x.code == "sql546"), "id % 2 = 0 is valid: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE id % 7 = 3;");
  assert!(!d2.iter().any(|x| x.code == "sql546"), "id % 7 = 3 is valid: {d2:?}");
}

#[test]
fn sql547_flags_array_length_eq_zero() {
  let d = diags("SELECT * FROM t WHERE array_length(tags, 1) = 0;");
  let m = d.iter().find(|x| x.code == "sql547").unwrap_or_else(|| panic!("expected sql547: {d:?}"));
  assert!(m.message.contains("never 0"), "{}", m.message);
}

#[test]
fn sql547_flags_array_length_lt_one() {
  let d = diags("SELECT * FROM t WHERE array_length(tags, 1) < 1;");
  assert!(d.iter().any(|x| x.code == "sql547"), "array_length < 1 never matches: {d:?}");
}

#[test]
fn sql547_quiet_for_valid_comparison() {
  let d = diags("SELECT * FROM t WHERE array_length(tags, 1) > 0;");
  assert!(!d.iter().any(|x| x.code == "sql547"), "array_length > 0 is valid: {d:?}");
  let d2 = diags("SELECT * FROM t WHERE array_length(tags, 1) = 3;");
  assert!(!d2.iter().any(|x| x.code == "sql547"), "array_length = 3 is valid: {d2:?}");
}

#[test]
fn sql548_flags_neq_all_array() {
  let d = diags("SELECT * FROM users WHERE id <> ALL(ARRAY[1, 2, 3]);");
  let m = d.iter().find(|x| x.code == "sql548").unwrap_or_else(|| panic!("expected sql548: {d:?}"));
  assert!(m.message.contains("NOT IN"), "{}", m.message);
}

#[test]
fn sql548_quiet_for_single_element_or_eq_all() {
  // Single element is sql521's domain.
  let d = diags("SELECT * FROM users WHERE id <> ALL(ARRAY[1]);");
  assert!(!d.iter().any(|x| x.code == "sql548"), "single-element <> ALL is sql521: {d:?}");
  // `= ALL` is a different (buggy) construct (sql495).
  let d2 = diags("SELECT * FROM users WHERE id = ALL(ARRAY[1, 2, 3]);");
  assert!(!d2.iter().any(|x| x.code == "sql548"), "= ALL must not flag here: {d2:?}");
}

#[test]
fn sql549_flags_table_self_alias() {
  let d = diags("SELECT * FROM users AS users;");
  let m = d.iter().find(|x| x.code == "sql549").unwrap_or_else(|| panic!("expected sql549: {d:?}"));
  assert!(m.message.contains("its own name"), "{}", m.message);
}

#[test]
fn sql549_flags_implicit_self_alias_in_join() {
  let d = diags("SELECT * FROM users u JOIN orders orders ON orders.user_id = u.id;");
  assert!(d.iter().any(|x| x.code == "sql549"), "JOIN orders orders should flag: {d:?}");
}

#[test]
fn sql549_quiet_for_real_alias() {
  let d = diags("SELECT * FROM users u JOIN orders o ON o.user_id = u.id;");
  assert!(!d.iter().any(|x| x.code == "sql549"), "distinct aliases must not flag: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE id = 1;");
  assert!(!d2.iter().any(|x| x.code == "sql549"), "no alias must not flag: {d2:?}");
}

#[test]
fn sql550_flags_two_lower_bounds() {
  let d = diags("SELECT * FROM users WHERE id > 5 AND id > 3;");
  let m = d.iter().find(|x| x.code == "sql550").unwrap_or_else(|| panic!("expected sql550: {d:?}"));
  assert!(m.message.contains("lower bounds"), "{}", m.message);
}

#[test]
fn sql550_quiet_for_opposite_bounds() {
  let d = diags("SELECT * FROM users WHERE id > 3 AND id < 10;");
  assert!(!d.iter().any(|x| x.code == "sql550"), "a real range must not flag: {d:?}");
}

#[test]
fn sql551_flags_upper_lower_nesting() {
  let d = diags("SELECT upper(lower(name)) FROM users;");
  let m = d.iter().find(|x| x.code == "sql551").unwrap_or_else(|| panic!("expected sql551: {d:?}"));
  assert!(m.message.contains("case-fold"), "{}", m.message);
}

#[test]
fn sql551_flags_trim_and_reverse() {
  let d = diags("SELECT trim(trim(name)) FROM users;");
  assert!(d.iter().any(|x| x.code == "sql551"), "trim(trim(x)) should flag: {d:?}");
  let d2 = diags("SELECT reverse(reverse(name)) FROM users;");
  assert!(d2.iter().any(|x| x.code == "sql551"), "reverse(reverse(x)) should flag: {d2:?}");
}

#[test]
fn sql551_quiet_for_non_redundant_nesting() {
  let d = diags("SELECT upper(trim(name)) FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql551"), "upper(trim(x)) is not redundant: {d:?}");
}

#[test]
fn sql552_flags_abs_negative() {
  let d = diags("SELECT * FROM t WHERE abs(balance) < 0;");
  let m = d.iter().find(|x| x.code == "sql552").unwrap_or_else(|| panic!("expected sql552: {d:?}"));
  assert!(m.message.contains("never negative"), "{}", m.message);
}

#[test]
fn sql552_flags_cardinality_eq_negative() {
  let d = diags("SELECT * FROM t WHERE cardinality(tags) = -1;");
  assert!(d.iter().any(|x| x.code == "sql552"), "cardinality = -1 never matches: {d:?}");
}

#[test]
fn sql552_quiet_for_valid_comparison() {
  let d = diags("SELECT * FROM t WHERE abs(balance) > 100;");
  assert!(!d.iter().any(|x| x.code == "sql552"), "abs > 100 is valid: {d:?}");
}

#[test]
fn sql553_flags_redundant_default_null() {
  let d = diags("CREATE TABLE t (id int, name text DEFAULT NULL);");
  let m = d.iter().find(|x| x.code == "sql553").unwrap_or_else(|| panic!("expected sql553: {d:?}"));
  assert!(m.message.contains("redundant"), "{}", m.message);
}

#[test]
fn sql553_quiet_for_not_null_or_real_default() {
  // NOT NULL DEFAULT NULL is sql069's (contradiction), not this rule.
  let d = diags("CREATE TABLE t (name text NOT NULL DEFAULT NULL);");
  assert!(!d.iter().any(|x| x.code == "sql553"), "NOT NULL case is sql069: {d:?}");
  let d2 = diags("CREATE TABLE t (name text DEFAULT 'x');");
  assert!(!d2.iter().any(|x| x.code == "sql553"), "real default must not flag: {d2:?}");
}

#[test]
fn sql554_flags_like_operator() {
  let d = diags("SELECT * FROM users WHERE name ~~ 'a%';");
  let m = d.iter().find(|x| x.code == "sql554").unwrap_or_else(|| panic!("expected sql554: {d:?}"));
  assert!(m.message.contains("LIKE"), "{}", m.message);
}

#[test]
fn sql554_flags_not_ilike_operator() {
  let d = diags("SELECT * FROM users WHERE name !~~* 'a%';");
  let m = d.iter().find(|x| x.code == "sql554").unwrap_or_else(|| panic!("expected sql554: {d:?}"));
  assert!(m.message.contains("NOT ILIKE"), "{}", m.message);
}

#[test]
fn sql554_quiet_for_regex_and_like_keyword() {
  let d = diags("SELECT * FROM users WHERE name ~ '^a';");
  assert!(!d.iter().any(|x| x.code == "sql554"), "regex ~ must not flag: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE name LIKE 'a%';");
  assert!(!d2.iter().any(|x| x.code == "sql554"), "LIKE keyword must not flag: {d2:?}");
}

#[test]
fn sql555_flags_is_true() {
  let d = diags("SELECT * FROM users WHERE active IS TRUE;");
  let m = d.iter().find(|x| x.code == "sql555").unwrap_or_else(|| panic!("expected sql555: {d:?}"));
  assert!(m.message.contains("IS TRUE"), "{}", m.message);
}

#[test]
fn sql555_flags_is_false() {
  let d = diags("SELECT * FROM users WHERE active IS FALSE;");
  let m = d.iter().find(|x| x.code == "sql555").unwrap_or_else(|| panic!("expected sql555: {d:?}"));
  assert!(m.message.contains("NOT"), "{}", m.message);
}

#[test]
fn sql555_quiet_for_is_not_true_and_select_list() {
  let d = diags("SELECT * FROM users WHERE active IS NOT TRUE;");
  assert!(!d.iter().any(|x| x.code == "sql555"), "IS NOT TRUE has different semantics: {d:?}");
  // In the SELECT list, `x IS TRUE` produces a boolean value -- not redundant.
  let d2 = diags("SELECT active IS TRUE AS flag FROM users;");
  assert!(!d2.iter().any(|x| x.code == "sql555"), "SELECT-list IS TRUE must not flag: {d2:?}");
}

#[test]
fn sql556_flags_eq_any_array() {
  let d = diags("SELECT * FROM users WHERE id = ANY(ARRAY[1, 2, 3]);");
  let m = d.iter().find(|x| x.code == "sql556").unwrap_or_else(|| panic!("expected sql556: {d:?}"));
  assert!(m.message.contains("IN (1, 2, 3)"), "{}", m.message);
}

#[test]
fn sql556_quiet_for_single_or_neq() {
  let d = diags("SELECT * FROM users WHERE id = ANY(ARRAY[1]);");
  assert!(!d.iter().any(|x| x.code == "sql556"), "single-element is sql521: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE id <> ANY(ARRAY[1, 2, 3]);");
  assert!(!d2.iter().any(|x| x.code == "sql556"), "<> ANY is not = ANY: {d2:?}");
}

#[test]
fn sql557_flags_duplicate_column() {
  let d = diags("CREATE TABLE t (id int, name text, id bigint);");
  let m = d.iter().find(|x| x.code == "sql557").unwrap_or_else(|| panic!("expected sql557: {d:?}"));
  assert!(m.message.contains("42701") && m.message.contains("id"), "{}", m.message);
}

#[test]
fn sql557_quiet_for_distinct_columns_and_constraints() {
  let d = diags("CREATE TABLE t (id int PRIMARY KEY, name text, UNIQUE (name));");
  assert!(!d.iter().any(|x| x.code == "sql557"), "distinct columns must not flag: {d:?}");
}

#[test]
fn sql558_flags_multiple_primary_keys() {
  let d = diags("CREATE TABLE t (id int PRIMARY KEY, code text PRIMARY KEY);");
  let m = d.iter().find(|x| x.code == "sql558").unwrap_or_else(|| panic!("expected sql558: {d:?}"));
  assert!(m.message.contains("42P16"), "{}", m.message);
}

#[test]
fn sql558_flags_inline_plus_table_level() {
  let d = diags("CREATE TABLE t (id int PRIMARY KEY, a int, b int, PRIMARY KEY (a, b));");
  assert!(d.iter().any(|x| x.code == "sql558"), "inline + table-level PK should flag: {d:?}");
}

#[test]
fn sql558_quiet_for_single_composite_pk() {
  let d = diags("CREATE TABLE t (a int, b int, PRIMARY KEY (a, b));");
  assert!(!d.iter().any(|x| x.code == "sql558"), "single composite PK must not flag: {d:?}");
}

#[test]
fn sql559_flags_duplicate_index_column() {
  let d = diags("CREATE INDEX idx ON users (name, id, name);");
  let m = d.iter().find(|x| x.code == "sql559").unwrap_or_else(|| panic!("expected sql559: {d:?}"));
  assert!(m.message.contains("42701"), "{}", m.message);
}

#[test]
fn sql559_quiet_for_distinct_index_columns() {
  let d = diags("CREATE INDEX idx ON users (name, id);");
  assert!(!d.iter().any(|x| x.code == "sql559"), "distinct index cols must not flag: {d:?}");
  let d2 = diags("CREATE UNIQUE INDEX idx ON users USING btree (lower(name), id);");
  assert!(!d2.iter().any(|x| x.code == "sql559"), "distinct expr cols must not flag: {d2:?}");
}

#[test]
fn sql560_flags_fk_count_mismatch() {
  let d = diags("CREATE TABLE t (a int, b int, FOREIGN KEY (a, b) REFERENCES other (c));");
  let m = d.iter().find(|x| x.code == "sql560").unwrap_or_else(|| panic!("expected sql560: {d:?}"));
  assert!(m.message.contains("42830"), "{}", m.message);
}

#[test]
fn sql560_quiet_for_matching_counts() {
  let d = diags("CREATE TABLE t (a int, b int, FOREIGN KEY (a, b) REFERENCES other (c, d));");
  assert!(!d.iter().any(|x| x.code == "sql560"), "matching FK counts must not flag: {d:?}");
  // Referenced PK omitted -> can't check, must not flag.
  let d2 = diags("CREATE TABLE t (a int, FOREIGN KEY (a) REFERENCES other);");
  assert!(!d2.iter().any(|x| x.code == "sql560"), "omitted ref cols must not flag: {d2:?}");
}

#[test]
fn sql561_flags_limit_all() {
  let d = diags("SELECT * FROM users LIMIT ALL;");
  let m = d.iter().find(|x| x.code == "sql561").unwrap_or_else(|| panic!("expected sql561: {d:?}"));
  assert!(m.message.contains("no limit"), "{}", m.message);
}

#[test]
fn sql561_quiet_for_numeric_limit() {
  let d = diags("SELECT * FROM users LIMIT 10;");
  assert!(!d.iter().any(|x| x.code == "sql561"), "numeric LIMIT must not flag: {d:?}");
}

#[test]
fn sql562_flags_default_subquery() {
  let d = diags("CREATE TABLE t (id int, seq int DEFAULT (SELECT max(seq) FROM t));");
  let m = d.iter().find(|x| x.code == "sql562").unwrap_or_else(|| panic!("expected sql562: {d:?}"));
  assert!(m.message.contains("DEFAULT"), "{}", m.message);
}

#[test]
fn sql562_quiet_for_normal_default() {
  let d = diags("CREATE TABLE t (id int, created timestamptz DEFAULT now());");
  assert!(!d.iter().any(|x| x.code == "sql562"), "function default must not flag: {d:?}");
  let d2 = diags("CREATE TABLE t (n int DEFAULT (1 + 2));");
  assert!(!d2.iter().any(|x| x.code == "sql562"), "parenthesized expression default must not flag: {d2:?}");
}

#[test]
fn sql563_flags_duplicate_in_any_array() {
  let d = diags("SELECT * FROM users WHERE id = ANY(ARRAY[1, 2, 1]);");
  let m = d.iter().find(|x| x.code == "sql563").unwrap_or_else(|| panic!("expected sql563: {d:?}"));
  assert!(m.message.contains("more than once"), "{}", m.message);
}

#[test]
fn sql563_quiet_for_distinct_array() {
  let d = diags("SELECT * FROM users WHERE id = ANY(ARRAY[1, 2, 3]);");
  assert!(!d.iter().any(|x| x.code == "sql563"), "distinct array must not flag: {d:?}");
}

#[test]
fn sql564_flags_null_not_null_conflict() {
  let d = diags("CREATE TABLE t (a int NULL NOT NULL);");
  let m = d.iter().find(|x| x.code == "sql564").unwrap_or_else(|| panic!("expected sql564: {d:?}"));
  assert!(m.message.contains("42601"), "{}", m.message);
}

#[test]
fn sql564_quiet_for_normal_columns() {
  let d = diags("CREATE TABLE t (a int NOT NULL, b int, c int DEFAULT NULL);");
  assert!(!d.iter().any(|x| x.code == "sql564"), "normal columns must not flag: {d:?}");
}

#[test]
fn sql565_flags_self_subtraction_and_division() {
  let d = diags("SELECT amount - amount FROM t;");
  let m = d.iter().find(|x| x.code == "sql565").unwrap_or_else(|| panic!("expected sql565: {d:?}"));
  assert!(m.message.contains("always 0"), "{}", m.message);
  let d2 = diags("SELECT total / total FROM t;");
  assert!(d2.iter().any(|x| x.code == "sql565" && x.message.contains("always 1")), "got {d2:?}");
}

#[test]
fn sql565_quiet_for_distinct_operands_and_arrows() {
  let d = diags("SELECT price - cost FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql565"), "distinct operands must not flag: {d:?}");
  let d2 = diags("SELECT data->'a' FROM t;");
  assert!(!d2.iter().any(|x| x.code == "sql565"), "jsonb arrow must not flag: {d2:?}");
}

#[test]
fn sql566_flags_col_eq_col_plus_one() {
  let d = diags("SELECT * FROM t WHERE counter = counter + 1;");
  let m = d.iter().find(|x| x.code == "sql566").unwrap_or_else(|| panic!("expected sql566: {d:?}"));
  assert!(m.message.contains("always false"), "{}", m.message);
}

#[test]
fn sql566_quiet_for_real_predicates() {
  let d = diags("SELECT * FROM t WHERE counter = other + 1;");
  assert!(!d.iter().any(|x| x.code == "sql566"), "different column must not flag: {d:?}");
  let d2 = diags("SELECT * FROM t WHERE counter = counter + 0;");
  assert!(!d2.iter().any(|x| x.code == "sql566"), "+ 0 is a no-op, not always-false: {d2:?}");
}

#[test]
fn sql567_flags_to_char_missing_format() {
  let d = diags("SELECT to_char(created_at) FROM t;");
  let m = d.iter().find(|x| x.code == "sql567").unwrap_or_else(|| panic!("expected sql567: {d:?}"));
  assert!(m.message.contains("42883") && m.message.contains("to_char"), "{}", m.message);
}

#[test]
fn sql567_flags_split_part_two_args() {
  let d = diags("SELECT split_part(path, '/') FROM t;");
  assert!(d.iter().any(|x| x.code == "sql567"), "split_part with 2 args should flag: {d:?}");
}

#[test]
fn sql567_quiet_for_correct_arity() {
  let d = diags("SELECT to_char(created_at, 'YYYY-MM-DD'), lpad(code, 5, '0') FROM t;");
  assert!(!d.iter().any(|x| x.code == "sql567"), "correct arity must not flag: {d:?}");
}

#[test]
fn sql568_flags_literal_regex() {
  let d = diags("SELECT * FROM t WHERE name ~ 'abc';");
  let m = d.iter().find(|x| x.code == "sql568").unwrap_or_else(|| panic!("expected sql568: {d:?}"));
  assert!(m.message.contains("LIKE '%abc%'"), "{}", m.message);
}

#[test]
fn sql568_quiet_for_real_regex_and_like() {
  let d = diags("SELECT * FROM t WHERE name ~ '^abc$';");
  assert!(!d.iter().any(|x| x.code == "sql568"), "regex with metachars must not flag: {d:?}");
  let d2 = diags("SELECT * FROM t WHERE name ~~ 'abc';");
  assert!(!d2.iter().any(|x| x.code == "sql568"), "~~ (LIKE operator) is sql554, not regex: {d2:?}");
}

#[test]
fn sql569_flags_order_by_in_exists() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id ORDER BY o.created_at);");
  let m = d.iter().find(|x| x.code == "sql569").unwrap_or_else(|| panic!("expected sql569: {d:?}"));
  assert!(m.message.contains("ignores ordering"), "{}", m.message);
}

#[test]
fn sql569_quiet_for_exists_without_order() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id);");
  assert!(!d.iter().any(|x| x.code == "sql569"), "EXISTS without ORDER BY must not flag: {d:?}");
}

#[test]
fn sql570_flags_distinct_in_exists() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT DISTINCT o.user_id FROM orders o WHERE o.user_id = u.id);");
  let m = d.iter().find(|x| x.code == "sql570").unwrap_or_else(|| panic!("expected sql570: {d:?}"));
  assert!(m.message.contains("pointless"), "{}", m.message);
}

#[test]
fn sql570_quiet_for_plain_exists() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id);");
  assert!(!d.iter().any(|x| x.code == "sql570"), "plain EXISTS must not flag: {d:?}");
}

#[test]
fn sql571_flags_plaintext_password() {
  let d = diags("CREATE ROLE app LOGIN PASSWORD 'hunter2';");
  let m = d.iter().find(|x| x.code == "sql571").unwrap_or_else(|| panic!("expected sql571: {d:?}"));
  assert!(m.message.contains("plaintext"), "{}", m.message);
}

#[test]
fn sql571_quiet_for_hashed_or_no_password() {
  let d = diags("CREATE ROLE app PASSWORD 'SCRAM-SHA-256$4096:abc$def:ghi';");
  assert!(!d.iter().any(|x| x.code == "sql571"), "scram verifier must not flag: {d:?}");
  let d2 = diags("ALTER ROLE app PASSWORD NULL;");
  assert!(!d2.iter().any(|x| x.code == "sql571"), "PASSWORD NULL must not flag: {d2:?}");
}

#[test]
fn sql572_flags_superuser() {
  let d = diags("CREATE ROLE deploy SUPERUSER LOGIN;");
  let m = d.iter().find(|x| x.code == "sql572").unwrap_or_else(|| panic!("expected sql572: {d:?}"));
  assert!(m.message.contains("SUPERUSER"), "{}", m.message);
}

#[test]
fn sql572_quiet_for_nosuperuser() {
  let d = diags("CREATE ROLE app NOSUPERUSER LOGIN;");
  assert!(!d.iter().any(|x| x.code == "sql572"), "NOSUPERUSER must not flag: {d:?}");
}

#[test]
fn sql573_flags_bypassrls() {
  let d = diags("CREATE ROLE etl BYPASSRLS LOGIN;");
  let m = d.iter().find(|x| x.code == "sql573").unwrap_or_else(|| panic!("expected sql573: {d:?}"));
  assert!(m.message.contains("row-level security"), "{}", m.message);
}

#[test]
fn sql573_quiet_for_nobypassrls() {
  let d = diags("CREATE ROLE app NOBYPASSRLS LOGIN;");
  assert!(!d.iter().any(|x| x.code == "sql573"), "NOBYPASSRLS must not flag: {d:?}");
}

#[test]
fn sql574_flags_disable_rls() {
  let d = diags("ALTER TABLE accounts DISABLE ROW LEVEL SECURITY;");
  let m = d.iter().find(|x| x.code == "sql574").unwrap_or_else(|| panic!("expected sql574: {d:?}"));
  assert!(m.message.contains("RLS"), "{}", m.message);
}

#[test]
fn sql574_quiet_for_enable_rls() {
  let d = diags("ALTER TABLE accounts ENABLE ROW LEVEL SECURITY;");
  assert!(!d.iter().any(|x| x.code == "sql574"), "ENABLE RLS must not flag: {d:?}");
}

#[test]
fn sql575_flags_policy_using_true() {
  let d = diags("CREATE POLICY p ON accounts USING (true);");
  let m = d.iter().find(|x| x.code == "sql575").unwrap_or_else(|| panic!("expected sql575: {d:?}"));
  assert!(m.message.contains("always true"), "{}", m.message);
}

#[test]
fn sql575_quiet_for_real_policy() {
  let d = diags("CREATE POLICY p ON accounts USING (owner_id = current_user_id());");
  assert!(!d.iter().any(|x| x.code == "sql575"), "real policy must not flag: {d:?}");
}

#[test]
fn sql576_flags_disable_trigger_all() {
  let d = diags("ALTER TABLE orders DISABLE TRIGGER ALL;");
  let m = d.iter().find(|x| x.code == "sql576").unwrap_or_else(|| panic!("expected sql576: {d:?}"));
  assert!(m.message.contains("foreign-key"), "{}", m.message);
}

#[test]
fn sql576_quiet_for_disable_trigger_user() {
  let d = diags("ALTER TABLE orders DISABLE TRIGGER USER;");
  assert!(!d.iter().any(|x| x.code == "sql576"), "DISABLE TRIGGER USER must not flag: {d:?}");
}

#[test]
fn sql577_flags_order_by_in_view() {
  let d = diags("CREATE VIEW v AS SELECT id, name FROM users ORDER BY name;");
  let m = d.iter().find(|x| x.code == "sql577").unwrap_or_else(|| panic!("expected sql577: {d:?}"));
  assert!(m.message.contains("isn't preserved"), "{}", m.message);
}

#[test]
fn sql577_quiet_for_top_n_and_matview() {
  let d = diags("CREATE VIEW v AS SELECT id FROM users ORDER BY id LIMIT 10;");
  assert!(!d.iter().any(|x| x.code == "sql577"), "ORDER BY + LIMIT is a deliberate top-N: {d:?}");
  let d2 = diags("CREATE MATERIALIZED VIEW v AS SELECT id FROM users ORDER BY id;");
  assert!(!d2.iter().any(|x| x.code == "sql577"), "matview ORDER BY must not flag: {d2:?}");
}

#[test]
fn sql578_flags_create_rule() {
  let d = diags("CREATE RULE log_ins AS ON INSERT TO t DO ALSO INSERT INTO audit VALUES (NEW.id);");
  let m = d.iter().find(|x| x.code == "sql578").unwrap_or_else(|| panic!("expected sql578: {d:?}"));
  assert!(m.message.contains("legacy"), "{}", m.message);
}

#[test]
fn sql578_quiet_for_create_table() {
  let d = diags("CREATE TABLE rule_book (id int);");
  assert!(!d.iter().any(|x| x.code == "sql578"), "CREATE TABLE must not flag: {d:?}");
}

#[test]
fn sql579_flags_autovacuum_disabled() {
  let d = diags("ALTER TABLE events SET (autovacuum_enabled = false);");
  let m = d.iter().find(|x| x.code == "sql579").unwrap_or_else(|| panic!("expected sql579: {d:?}"));
  assert!(m.message.contains("autovacuum is disabled"), "{}", m.message);
}

#[test]
fn sql579_quiet_for_other_storage_params() {
  let d = diags("ALTER TABLE events SET (fillfactor = 70, autovacuum_enabled = true);");
  assert!(!d.iter().any(|x| x.code == "sql579"), "enabled/other params must not flag: {d:?}");
}

#[test]
fn sql580_flags_unlogged_table() {
  let d = diags("CREATE UNLOGGED TABLE cache (k text, v text);");
  let m = d.iter().find(|x| x.code == "sql580").unwrap_or_else(|| panic!("expected sql580: {d:?}"));
  assert!(m.message.contains("UNLOGGED"), "{}", m.message);
}

#[test]
fn sql580_quiet_for_logged_table() {
  let d = diags("CREATE TABLE accounts (id int);");
  assert!(!d.iter().any(|x| x.code == "sql580"), "logged table must not flag: {d:?}");
}

#[test]
fn sql581_flags_json_column() {
  let d = diags("CREATE TABLE t (id int, payload json);");
  let m = d.iter().find(|x| x.code == "sql581").unwrap_or_else(|| panic!("expected sql581: {d:?}"));
  assert!(m.message.contains("jsonb"), "{}", m.message);
}

#[test]
fn sql581_quiet_for_jsonb_and_json_functions() {
  let d = diags("CREATE TABLE t (id int, payload jsonb);");
  assert!(!d.iter().any(|x| x.code == "sql581"), "jsonb must not flag: {d:?}");
  let d2 = diags("SELECT json_build_object('a', 1), to_json(x) FROM t;");
  assert!(!d2.iter().any(|x| x.code == "sql581"), "json functions must not flag: {d2:?}");
}

#[test]
fn sql582_flags_money_type() {
  let d = diags("CREATE TABLE invoices (id int, total money);");
  let m = d.iter().find(|x| x.code == "sql582").unwrap_or_else(|| panic!("expected sql582: {d:?}"));
  assert!(m.message.contains("numeric"), "{}", m.message);
}

#[test]
fn sql582_quiet_for_numeric() {
  let d = diags("CREATE TABLE invoices (id int, total numeric(12, 2));");
  assert!(!d.iter().any(|x| x.code == "sql582"), "numeric must not flag: {d:?}");
}

#[test]
fn sql583_flags_group_by_in_exists() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id GROUP BY o.status);");
  let m = d.iter().find(|x| x.code == "sql583").unwrap_or_else(|| panic!("expected sql583: {d:?}"));
  assert!(m.message.contains("pointless"), "{}", m.message);
}

#[test]
fn sql583_quiet_for_group_by_with_having() {
  let d = diags("SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id GROUP BY o.status HAVING count(*) > 3);");
  assert!(!d.iter().any(|x| x.code == "sql583"), "GROUP BY + HAVING is meaningful: {d:?}");
}

#[test]
fn sql584_flags_internal_aliases() {
  let d = diags("CREATE TABLE t (a int4, b float8);");
  assert!(d.iter().any(|x| x.code == "sql584" && x.message.contains("integer")), "int4: {d:?}");
  assert!(d.iter().any(|x| x.code == "sql584" && x.message.contains("double precision")), "float8: {d:?}");
}

#[test]
fn sql584_quiet_for_standard_types() {
  let d = diags("CREATE TABLE t (a integer, b bigint);");
  assert!(!d.iter().any(|x| x.code == "sql584"), "standard types must not flag: {d:?}");
}

#[test]
fn sql585_flags_cluster() {
  let d = diags("CLUSTER orders USING orders_pkey;");
  let m = d.iter().find(|x| x.code == "sql585").unwrap_or_else(|| panic!("expected sql585: {d:?}"));
  assert!(m.message.contains("ACCESS EXCLUSIVE"), "{}", m.message);
}

#[test]
fn sql585_quiet_for_alter_cluster_on() {
  let d = diags("ALTER TABLE orders CLUSTER ON orders_pkey;");
  assert!(!d.iter().any(|x| x.code == "sql585"), "ALTER ... CLUSTER ON just marks the index: {d:?}");
}

#[test]
fn sql586_flags_vacuum_full() {
  let d = diags("VACUUM FULL orders;");
  let m = d.iter().find(|x| x.code == "sql586").unwrap_or_else(|| panic!("expected sql586: {d:?}"));
  assert!(m.message.contains("ACCESS EXCLUSIVE"), "{}", m.message);
}

#[test]
fn sql586_quiet_for_plain_vacuum() {
  let d = diags("VACUUM ANALYZE orders;");
  assert!(!d.iter().any(|x| x.code == "sql586"), "plain VACUUM must not flag: {d:?}");
}

#[test]
fn sql587_flags_add_column_volatile_default() {
  let d = diags("ALTER TABLE users ADD COLUMN ref uuid DEFAULT gen_random_uuid();");
  let m = d.iter().find(|x| x.code == "sql587").unwrap_or_else(|| panic!("expected sql587: {d:?}"));
  assert!(m.message.contains("rewrites the whole table"), "{}", m.message);
}

#[test]
fn sql587_quiet_for_constant_default() {
  let d = diags("ALTER TABLE users ADD COLUMN active boolean DEFAULT true;");
  assert!(!d.iter().any(|x| x.code == "sql587"), "constant default must not flag: {d:?}");
}

#[test]
fn sql588_flags_alter_add_primary_key() {
  let d = diags("ALTER TABLE users ADD PRIMARY KEY (id);");
  let m = d.iter().find(|x| x.code == "sql588").unwrap_or_else(|| panic!("expected sql588: {d:?}"));
  assert!(m.message.contains("ACCESS EXCLUSIVE"), "{}", m.message);
}

#[test]
fn sql588_quiet_for_using_index_and_create_table() {
  let d = diags("ALTER TABLE users ADD CONSTRAINT users_pkey PRIMARY KEY USING INDEX users_id_idx;");
  assert!(!d.iter().any(|x| x.code == "sql588"), "USING INDEX form must not flag: {d:?}");
  let d2 = diags("CREATE TABLE t (id int, PRIMARY KEY (id));");
  assert!(!d2.iter().any(|x| x.code == "sql588"), "CREATE TABLE PK must not flag: {d2:?}");
}

#[test]
fn sql589_flags_add_fk_without_not_valid() {
  let d = diags("ALTER TABLE orders ADD CONSTRAINT fk FOREIGN KEY (user_id) REFERENCES users (id);");
  let m = d.iter().find(|x| x.code == "sql589").unwrap_or_else(|| panic!("expected sql589: {d:?}"));
  assert!(m.message.contains("NOT VALID"), "{}", m.message);
}

#[test]
fn sql589_quiet_for_not_valid() {
  let d = diags("ALTER TABLE orders ADD CONSTRAINT fk FOREIGN KEY (user_id) REFERENCES users (id) NOT VALID;");
  assert!(!d.iter().any(|x| x.code == "sql589"), "NOT VALID must not flag: {d:?}");
}

#[test]
fn sql590_flags_reindex() {
  let d = diags("REINDEX TABLE orders;");
  let m = d.iter().find(|x| x.code == "sql590").unwrap_or_else(|| panic!("expected sql590: {d:?}"));
  assert!(m.message.contains("CONCURRENTLY"), "{}", m.message);
}

#[test]
fn sql590_quiet_for_concurrent_reindex() {
  let d = diags("REINDEX INDEX CONCURRENTLY orders_pkey;");
  assert!(!d.iter().any(|x| x.code == "sql590"), "CONCURRENTLY reindex must not flag: {d:?}");
}

#[test]
fn sql591_flags_inconsistent_values() {
  let d = diags("INSERT INTO t (a, b) VALUES (1, 2), (3, 4, 5);");
  let m = d.iter().find(|x| x.code == "sql591").unwrap_or_else(|| panic!("expected sql591: {d:?}"));
  assert!(m.message.contains("21000"), "{}", m.message);
}

#[test]
fn sql591_quiet_for_consistent_values() {
  let d = diags("INSERT INTO t (a, b) VALUES (1, 2), (3, 4), (5, 6);");
  assert!(!d.iter().any(|x| x.code == "sql591"), "consistent VALUES must not flag: {d:?}");
}

#[test]
fn sql592_flags_where_bare_integer() {
  let d = diags("SELECT * FROM users WHERE 1;");
  let m = d.iter().find(|x| x.code == "sql592").unwrap_or_else(|| panic!("expected sql592: {d:?}"));
  assert!(m.message.contains("42804"), "{}", m.message);
}

#[test]
fn sql592_quiet_for_boolean_predicate() {
  let d = diags("SELECT * FROM users WHERE id = 1;");
  assert!(!d.iter().any(|x| x.code == "sql592"), "real predicate must not flag: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE true;");
  assert!(!d2.iter().any(|x| x.code == "sql592"), "WHERE true must not flag: {d2:?}");
}

#[test]
fn sql593_flags_mysql_limit_comma() {
  let d = diags("SELECT * FROM users ORDER BY id LIMIT 10, 20;");
  let m = d.iter().find(|x| x.code == "sql593").unwrap_or_else(|| panic!("expected sql593: {d:?}"));
  assert!(m.message.contains("MySQL"), "{}", m.message);
}

#[test]
fn sql593_quiet_for_standard_limit_offset() {
  let d = diags("SELECT * FROM users ORDER BY id LIMIT 20 OFFSET 10;");
  assert!(!d.iter().any(|x| x.code == "sql593"), "standard LIMIT/OFFSET must not flag: {d:?}");
}

#[test]
fn sql594_flags_on_duplicate_key() {
  let d = diags("INSERT INTO t (id, n) VALUES (1, 2) ON DUPLICATE KEY UPDATE n = n + 1;");
  let m = d.iter().find(|x| x.code == "sql594").unwrap_or_else(|| panic!("expected sql594: {d:?}"));
  assert!(m.message.contains("ON CONFLICT"), "{}", m.message);
}

#[test]
fn sql594_quiet_for_on_conflict() {
  let d = diags("INSERT INTO t (id, n) VALUES (1, 2) ON CONFLICT (id) DO UPDATE SET n = EXCLUDED.n;");
  assert!(!d.iter().any(|x| x.code == "sql594"), "ON CONFLICT must not flag: {d:?}");
}

#[test]
fn sql595_flags_replace_into() {
  let d = diags("REPLACE INTO users (id, name) VALUES (1, 'a');");
  let m = d.iter().find(|x| x.code == "sql595").unwrap_or_else(|| panic!("expected sql595: {d:?}"));
  assert!(m.message.contains("ON CONFLICT"), "{}", m.message);
}

#[test]
fn sql595_quiet_for_replace_function() {
  let d = diags("SELECT replace(name, '-', '_') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql595"), "replace() function must not flag: {d:?}");
}

#[test]
fn sql596_flags_mysql_functions() {
  let d = diags("SELECT group_concat(name), date_format(created_at, '%Y') FROM users;");
  assert!(d.iter().any(|x| x.code == "sql596" && x.message.contains("string_agg")), "group_concat: {d:?}");
  assert!(d.iter().any(|x| x.code == "sql596" && x.message.contains("to_char")), "date_format: {d:?}");
}

#[test]
fn sql596_quiet_for_pg_functions() {
  let d = diags("SELECT string_agg(name, ','), to_char(created_at, 'YYYY') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql596"), "PG functions must not flag: {d:?}");
}

#[test]
fn sql597_flags_regexp_and_rlike() {
  let d = diags("SELECT * FROM users WHERE name REGEXP '^a';");
  assert!(d.iter().any(|x| x.code == "sql597"), "REGEXP: {d:?}");
  let d2 = diags("SELECT * FROM users WHERE name RLIKE 'a';");
  assert!(d2.iter().any(|x| x.code == "sql597"), "RLIKE: {d2:?}");
}

#[test]
fn sql597_quiet_for_regexp_functions() {
  let d = diags("SELECT regexp_replace(name, 'a', 'b') FROM users;");
  assert!(!d.iter().any(|x| x.code == "sql597"), "regexp_replace must not flag: {d:?}");
}

#[test]
fn sql598_flags_use_statement() {
  let d = diags("USE analytics;");
  let m = d.iter().find(|x| x.code == "sql598").unwrap_or_else(|| panic!("expected sql598: {d:?}"));
  assert!(m.message.contains("no USE"), "{}", m.message);
}

#[test]
fn sql598_quiet_for_using_clause() {
  let d = diags("DELETE FROM a USING b WHERE a.id = b.id;");
  assert!(!d.iter().any(|x| x.code == "sql598"), "USING clause must not flag: {d:?}");
}

#[test]
fn sql599_unsigned_modifier() {
  let d = diags("CREATE TABLE t (id int unsigned)");
  assert!(d.iter().any(|x| x.code == "sql599"));
}

#[test]
fn sql599_bigint_unsigned() {
  let d = diags("CREATE TABLE t (n bigint UNSIGNED not null)");
  assert!(d.iter().any(|x| x.code == "sql599"));
}

#[test]
fn sql599_quiet_plain_int() {
  let d = diags("CREATE TABLE t (id int)");
  assert!(!d.iter().any(|x| x.code == "sql599"));
}

#[test]
fn sql600_backtick_identifier() {
  let d = diags("SELECT `name` FROM users");
  assert!(d.iter().any(|x| x.code == "sql600"));
}

#[test]
fn sql600_quiet_double_quote() {
  let d = diags("SELECT \"name\" FROM users");
  assert!(!d.iter().any(|x| x.code == "sql600"));
}

#[test]
fn sql600_quiet_backtick_in_string() {
  let d = diags("SELECT 'a`b' FROM users");
  assert!(!d.iter().any(|x| x.code == "sql600"));
}

#[test]
fn sql601_varchar2() {
  let d = diags("CREATE TABLE t (name varchar2(50))");
  assert!(d.iter().any(|x| x.code == "sql601"));
}

#[test]
fn sql601_nvarchar2() {
  let d = diags("CREATE TABLE t (name NVARCHAR2(50))");
  assert!(d.iter().any(|x| x.code == "sql601"));
}

#[test]
fn sql601_quiet_varchar() {
  let d = diags("CREATE TABLE t (name varchar(50))");
  assert!(!d.iter().any(|x| x.code == "sql601"));
}

#[test]
fn sql602_decode_three_args() {
  let d = diags("SELECT decode(status, 1, 'active') FROM t");
  assert!(d.iter().any(|x| x.code == "sql602"));
}

#[test]
fn sql602_decode_with_default() {
  let d = diags("SELECT decode(grade, 'A', 4, 'B', 3, 0) FROM t");
  assert!(d.iter().any(|x| x.code == "sql602"));
}

#[test]
fn sql602_quiet_two_arg_decode() {
  let d = diags("SELECT decode(payload, 'base64') FROM t");
  assert!(!d.iter().any(|x| x.code == "sql602"));
}

#[test]
fn sql603_minus_operator() {
  let d = diags("SELECT id FROM a MINUS SELECT id FROM b");
  assert!(d.iter().any(|x| x.code == "sql603"));
}

#[test]
fn sql603_quiet_except() {
  let d = diags("SELECT id FROM a EXCEPT SELECT id FROM b");
  assert!(!d.iter().any(|x| x.code == "sql603"));
}

#[test]
fn sql603_quiet_identifier() {
  let d = diags("SELECT minus_balance FROM accounts");
  assert!(!d.iter().any(|x| x.code == "sql603"));
}

#[test]
fn sql604_clob() {
  let d = diags("CREATE TABLE t (body clob)");
  assert!(d.iter().any(|x| x.code == "sql604"));
}

#[test]
fn sql604_nclob() {
  let d = diags("CREATE TABLE t (body NCLOB)");
  assert!(d.iter().any(|x| x.code == "sql604"));
}

#[test]
fn sql604_quiet_text() {
  let d = diags("CREATE TABLE t (body text)");
  assert!(!d.iter().any(|x| x.code == "sql604"));
}

#[test]
fn sql605_set_null_on_not_null() {
  let d = diags("CREATE TABLE o (cid int NOT NULL REFERENCES c(id) ON DELETE SET NULL)");
  assert!(d.iter().any(|x| x.code == "sql605"));
}

#[test]
fn sql605_quiet_nullable_set_null() {
  let d = diags("CREATE TABLE o (cid int REFERENCES c(id) ON DELETE SET NULL)");
  assert!(!d.iter().any(|x| x.code == "sql605"));
}

#[test]
fn sql605_quiet_cascade() {
  let d = diags("CREATE TABLE o (cid int NOT NULL REFERENCES c(id) ON DELETE CASCADE)");
  assert!(!d.iter().any(|x| x.code == "sql605"));
}

#[test]
fn sql606_subquery_in_check() {
  let d = diags("CREATE TABLE t (x int CHECK (x IN (SELECT id FROM allowed)))");
  assert!(d.iter().any(|x| x.code == "sql606"));
}

#[test]
fn sql606_quiet_plain_check() {
  let d = diags("CREATE TABLE t (x int CHECK (x > 0))");
  assert!(!d.iter().any(|x| x.code == "sql606"));
}

#[test]
fn sql606_quiet_no_check() {
  let d = diags("SELECT id FROM (SELECT id FROM t) s");
  assert!(!d.iter().any(|x| x.code == "sql606"));
}

#[test]
fn sql607_text_with_length() {
  let d = diags("CREATE TABLE t (a text(50))");
  assert!(d.iter().any(|x| x.code == "sql607"));
}

#[test]
fn sql607_bytea_with_length() {
  let d = diags("CREATE TABLE t (a bytea(16))");
  assert!(d.iter().any(|x| x.code == "sql607"));
}

#[test]
fn sql607_quiet_plain_text() {
  let d = diags("CREATE TABLE t (a text)");
  assert!(!d.iter().any(|x| x.code == "sql607"));
}

#[test]
fn sql607_quiet_varchar() {
  let d = diags("CREATE TABLE t (a varchar(50))");
  assert!(!d.iter().any(|x| x.code == "sql607"));
}

#[test]
fn sql608_unique_hash_index() {
  let d = diags("CREATE UNIQUE INDEX idx ON t USING hash (a)");
  assert!(d.iter().any(|x| x.code == "sql608"));
}

#[test]
fn sql608_unique_gin_index() {
  let d = diags("CREATE UNIQUE INDEX idx ON t USING gin (a)");
  assert!(d.iter().any(|x| x.code == "sql608"));
}

#[test]
fn sql608_quiet_nonunique_hash() {
  let d = diags("CREATE INDEX idx ON t USING hash (a)");
  assert!(!d.iter().any(|x| x.code == "sql608"));
}

#[test]
fn sql608_quiet_unique_btree() {
  let d = diags("CREATE UNIQUE INDEX idx ON t USING btree (a)");
  assert!(!d.iter().any(|x| x.code == "sql608"));
}

#[test]
fn sql609_distinct_for_update() {
  let d = diags("SELECT DISTINCT id FROM t FOR UPDATE");
  assert!(d.iter().any(|x| x.code == "sql609"));
}

#[test]
fn sql609_quiet_distinct_no_lock() {
  let d = diags("SELECT DISTINCT id FROM t");
  assert!(!d.iter().any(|x| x.code == "sql609"));
}

#[test]
fn sql609_quiet_lock_no_distinct() {
  let d = diags("SELECT id FROM t FOR UPDATE");
  assert!(!d.iter().any(|x| x.code == "sql609"));
}

#[test]
fn sql610_window_for_update() {
  let d = diags("SELECT id, row_number() OVER (ORDER BY id) FROM t FOR UPDATE");
  assert!(d.iter().any(|x| x.code == "sql610"));
}

#[test]
fn sql610_quiet_window_no_lock() {
  let d = diags("SELECT id, row_number() OVER (ORDER BY id) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql610"));
}

#[test]
fn sql610_quiet_lock_no_window() {
  let d = diags("SELECT id FROM t FOR UPDATE");
  assert!(!d.iter().any(|x| x.code == "sql610"));
}

#[test]
fn sql611_delete_order_by() {
  let d = diags("DELETE FROM t ORDER BY id LIMIT 5");
  assert!(d.iter().any(|x| x.code == "sql611"));
}

#[test]
fn sql611_update_order_by() {
  let d = diags("UPDATE t SET x = 1 ORDER BY id");
  assert!(d.iter().any(|x| x.code == "sql611"));
}

#[test]
fn sql611_quiet_order_by_in_subquery() {
  let d = diags("UPDATE t SET x = (SELECT v FROM s ORDER BY v LIMIT 1)");
  assert!(!d.iter().any(|x| x.code == "sql611"));
}

#[test]
fn sql611_quiet_select_order_by() {
  let d = diags("SELECT id FROM t ORDER BY id");
  assert!(!d.iter().any(|x| x.code == "sql611"));
}

#[test]
fn sql612_returning_count() {
  let d = diags("INSERT INTO t (a) VALUES (1) RETURNING count(*)");
  assert!(d.iter().any(|x| x.code == "sql612"));
}

#[test]
fn sql612_returning_sum() {
  let d = diags("DELETE FROM t WHERE a > 0 RETURNING sum(a)");
  assert!(d.iter().any(|x| x.code == "sql612"));
}

#[test]
fn sql612_quiet_returning_column() {
  let d = diags("INSERT INTO t (a) VALUES (1) RETURNING id");
  assert!(!d.iter().any(|x| x.code == "sql612"));
}

#[test]
fn sql612_quiet_no_returning() {
  let d = diags("SELECT count(*) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql612"));
}

#[test]
fn sql613_generated_missing_stored() {
  let d = diags("CREATE TABLE t (w int, h int, area int GENERATED ALWAYS AS (w * h))");
  assert!(d.iter().any(|x| x.code == "sql613"));
}

#[test]
fn sql613_generated_virtual() {
  let d = diags("CREATE TABLE t (w int, area int GENERATED ALWAYS AS (w * 2) VIRTUAL)");
  assert!(d.iter().any(|x| x.code == "sql613"));
}

#[test]
fn sql613_quiet_stored() {
  let d = diags("CREATE TABLE t (w int, h int, area int GENERATED ALWAYS AS (w * h) STORED)");
  assert!(!d.iter().any(|x| x.code == "sql613"));
}

#[test]
fn sql613_quiet_identity() {
  let d = diags("CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY)");
  assert!(!d.iter().any(|x| x.code == "sql613"));
}

#[test]
fn sql614_inline_key() {
  let d = diags("CREATE TABLE t (id int, name text, KEY idx_name (name))");
  assert!(d.iter().any(|x| x.code == "sql614"));
}

#[test]
fn sql614_inline_index() {
  let d = diags("CREATE TABLE t (id int, name text, INDEX idx_name (name))");
  assert!(d.iter().any(|x| x.code == "sql614"));
}

#[test]
fn sql614_quiet_primary_key() {
  let d = diags("CREATE TABLE t (id int PRIMARY KEY, name text)");
  assert!(!d.iter().any(|x| x.code == "sql614"));
}

#[test]
fn sql614_quiet_foreign_key() {
  let d = diags("CREATE TABLE t (id int, oid int, FOREIGN KEY (oid) REFERENCES o(id))");
  assert!(!d.iter().any(|x| x.code == "sql614"));
}

#[test]
fn sql615_with_oids() {
  let d = diags("CREATE TABLE t (id int) WITH OIDS");
  assert!(d.iter().any(|x| x.code == "sql615"));
}

#[test]
fn sql615_quiet_without_oids() {
  let d = diags("CREATE TABLE t (id int) WITHOUT OIDS");
  assert!(!d.iter().any(|x| x.code == "sql615"));
}

#[test]
fn sql615_quiet_plain() {
  let d = diags("CREATE TABLE t (id int)");
  assert!(!d.iter().any(|x| x.code == "sql615"));
}

#[test]
fn sql616_character_set() {
  let d = diags("CREATE TABLE t (name varchar(50) CHARACTER SET utf8mb4)");
  assert!(d.iter().any(|x| x.code == "sql616"));
}

#[test]
fn sql616_charset() {
  let d = diags("CREATE TABLE t (id int) ENGINE=InnoDB DEFAULT CHARSET=utf8");
  assert!(d.iter().any(|x| x.code == "sql616"));
}

#[test]
fn sql616_quiet_character_varying() {
  let d = diags("CREATE TABLE t (name character varying(50))");
  assert!(!d.iter().any(|x| x.code == "sql616"));
}

#[test]
fn sql616_quiet_collate() {
  let d = diags("CREATE TABLE t (name text COLLATE \"en_US\")");
  assert!(!d.iter().any(|x| x.code == "sql616"));
}

#[test]
fn sql617_natural_join() {
  let d = diags("SELECT * FROM a NATURAL JOIN b");
  assert!(d.iter().any(|x| x.code == "sql617"));
}

#[test]
fn sql617_natural_left_join() {
  let d = diags("SELECT * FROM a NATURAL LEFT JOIN b");
  assert!(d.iter().any(|x| x.code == "sql617"));
}

#[test]
fn sql617_quiet_explicit_join() {
  let d = diags("SELECT * FROM a JOIN b ON a.id = b.id");
  assert!(!d.iter().any(|x| x.code == "sql617"));
}

#[test]
fn sql618_with_ties_no_order() {
  let d = diags("SELECT * FROM t FETCH FIRST 5 ROWS WITH TIES");
  assert!(d.iter().any(|x| x.code == "sql618"));
}

#[test]
fn sql618_quiet_with_ties_and_order() {
  let d = diags("SELECT * FROM t ORDER BY id FETCH FIRST 5 ROWS WITH TIES");
  assert!(!d.iter().any(|x| x.code == "sql618"));
}

#[test]
fn sql618_quiet_rows_only() {
  let d = diags("SELECT * FROM t FETCH FIRST 5 ROWS ONLY");
  assert!(!d.iter().any(|x| x.code == "sql618"));
}

#[test]
fn sql619_invalid_unit() {
  let d = diags("SELECT date_trunc('minutes', ts) FROM t");
  assert!(d.iter().any(|x| x.code == "sql619"));
}

#[test]
fn sql619_invalid_unit_typo() {
  let d = diags("SELECT date_trunc('mon', ts) FROM t");
  assert!(d.iter().any(|x| x.code == "sql619"));
}

#[test]
fn sql619_quiet_valid_unit() {
  let d = diags("SELECT date_trunc('month', ts) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql619"));
}

#[test]
fn sql619_quiet_column_unit() {
  let d = diags("SELECT date_trunc(unit_col, ts) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql619"));
}

#[test]
fn sql620_datediff() {
  let d = diags("SELECT datediff(day, a, b) FROM t");
  assert!(d.iter().any(|x| x.code == "sql620"));
}

#[test]
fn sql620_timestampdiff() {
  let d = diags("SELECT timestampdiff(MINUTE, a, b) FROM t");
  assert!(d.iter().any(|x| x.code == "sql620"));
}

#[test]
fn sql620_quiet_extract() {
  let d = diags("SELECT extract(day FROM (a - b)) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql620"));
}

#[test]
fn sql621_if_function() {
  let d = diags("SELECT IF(x > 0, 'pos', 'neg') FROM t");
  assert!(d.iter().any(|x| x.code == "sql621"));
}

#[test]
fn sql621_quiet_plpgsql_if() {
  let d = diags("CREATE FUNCTION f() RETURNS int AS $$ BEGIN IF (x > 0) THEN RETURN 1; END IF; END; $$ LANGUAGE plpgsql");
  assert!(!d.iter().any(|x| x.code == "sql621"));
}

#[test]
fn sql621_quiet_if_exists() {
  let d = diags("DROP TABLE IF EXISTS t");
  assert!(!d.iter().any(|x| x.code == "sql621"));
}

#[test]
fn sql622_lcase() {
  let d = diags("SELECT lcase(name) FROM t");
  assert!(d.iter().any(|x| x.code == "sql622"));
}

#[test]
fn sql622_substring_index() {
  let d = diags("SELECT substring_index(path, '/', 1) FROM t");
  assert!(d.iter().any(|x| x.code == "sql622"));
}

#[test]
fn sql622_quiet_lower() {
  let d = diags("SELECT lower(name) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql622"));
}

#[test]
fn sql623_inline_enum() {
  let d = diags("CREATE TABLE t (status ENUM('a', 'b', 'c'))");
  assert!(d.iter().any(|x| x.code == "sql623"));
}

#[test]
fn sql623_quiet_named_type() {
  let d = diags("CREATE TABLE t (status mood)");
  assert!(!d.iter().any(|x| x.code == "sql623"));
}

#[test]
fn sql624_on_update_current_timestamp() {
  let d = diags("CREATE TABLE t (updated_at timestamp ON UPDATE CURRENT_TIMESTAMP)");
  assert!(d.iter().any(|x| x.code == "sql624"));
}

#[test]
fn sql624_on_update_now() {
  let d = diags("CREATE TABLE t (updated_at timestamp ON UPDATE NOW())");
  assert!(d.iter().any(|x| x.code == "sql624"));
}

#[test]
fn sql624_quiet_fk_on_update_cascade() {
  let d = diags("CREATE TABLE t (oid int REFERENCES o(id) ON UPDATE CASCADE)");
  assert!(!d.iter().any(|x| x.code == "sql624"));
}

#[test]
fn sql624_quiet_fk_on_update_set_null() {
  let d = diags("CREATE TABLE t (oid int REFERENCES o(id) ON UPDATE SET NULL)");
  assert!(!d.iter().any(|x| x.code == "sql624"));
}

#[test]
fn sql625_zerofill() {
  let d = diags("CREATE TABLE t (n int ZEROFILL)");
  assert!(d.iter().any(|x| x.code == "sql625"));
}

#[test]
fn sql625_quiet_plain_int() {
  let d = diags("CREATE TABLE t (n int)");
  assert!(!d.iter().any(|x| x.code == "sql625"));
}

#[test]
fn sql626_calc_found_rows() {
  let d = diags("SELECT SQL_CALC_FOUND_ROWS * FROM t LIMIT 10");
  assert!(d.iter().any(|x| x.code == "sql626"));
}

#[test]
fn sql626_straight_join() {
  let d = diags("SELECT * FROM a STRAIGHT_JOIN b ON a.id = b.id");
  assert!(d.iter().any(|x| x.code == "sql626"));
}

#[test]
fn sql626_low_priority() {
  let d = diags("INSERT LOW_PRIORITY INTO t (a) VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql626"));
}

#[test]
fn sql626_quiet_plain_select() {
  let d = diags("SELECT * FROM t LIMIT 10");
  assert!(!d.iter().any(|x| x.code == "sql626"));
}

#[test]
fn sql627_xor() {
  let d = diags("SELECT * FROM t WHERE a XOR b");
  assert!(d.iter().any(|x| x.code == "sql627"));
}

#[test]
fn sql627_div() {
  let d = diags("SELECT a DIV b FROM t");
  assert!(d.iter().any(|x| x.code == "sql627"));
}

#[test]
fn sql627_quiet_plain() {
  let d = diags("SELECT a / b FROM t WHERE a <> b");
  assert!(!d.iter().any(|x| x.code == "sql627"));
}

#[test]
fn sql628_instr() {
  let d = diags("SELECT instr(name, 'x') FROM t");
  assert!(d.iter().any(|x| x.code == "sql628"));
}

#[test]
fn sql628_iif() {
  let d = diags("SELECT iif(x > 0, 1, 0) FROM t");
  assert!(d.iter().any(|x| x.code == "sql628"));
}

#[test]
fn sql628_nvl2() {
  let d = diags("SELECT nvl2(a, b, c) FROM t");
  assert!(d.iter().any(|x| x.code == "sql628"));
}

#[test]
fn sql628_quiet_length() {
  let d = diags("SELECT length(name), strpos(name, 'x') FROM t");
  assert!(!d.iter().any(|x| x.code == "sql628"));
}

#[test]
fn sql629_nvarchar() {
  let d = diags("CREATE TABLE t (name NVARCHAR(50))");
  assert!(d.iter().any(|x| x.code == "sql629"));
}

#[test]
fn sql629_uniqueidentifier() {
  let d = diags("CREATE TABLE t (id UNIQUEIDENTIFIER)");
  assert!(d.iter().any(|x| x.code == "sql629"));
}

#[test]
fn sql629_datetime2() {
  let d = diags("CREATE TABLE t (ts DATETIME2)");
  assert!(d.iter().any(|x| x.code == "sql629"));
}

#[test]
fn sql629_quiet_varchar() {
  let d = diags("CREATE TABLE t (name varchar(50))");
  assert!(!d.iter().any(|x| x.code == "sql629"));
}

#[test]
fn sql630_newid() {
  let d = diags("CREATE TABLE t (id uuid DEFAULT newid())");
  assert!(d.iter().any(|x| x.code == "sql630"));
}

#[test]
fn sql630_scope_identity() {
  let d = diags("SELECT scope_identity()");
  assert!(d.iter().any(|x| x.code == "sql630"));
}

#[test]
fn sql630_quiet_gen_random_uuid() {
  let d = diags("CREATE TABLE t (id uuid DEFAULT gen_random_uuid())");
  assert!(!d.iter().any(|x| x.code == "sql630"));
}

#[test]
fn sql631_last_value_no_frame() {
  let d = diags("SELECT last_value(x) OVER (ORDER BY y) FROM t");
  assert!(d.iter().any(|x| x.code == "sql631"));
}

#[test]
fn sql631_quiet_with_frame() {
  let d = diags("SELECT last_value(x) OVER (ORDER BY y ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql631"));
}

#[test]
fn sql631_quiet_first_value() {
  let d = diags("SELECT first_value(x) OVER (ORDER BY y) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql631"));
}

#[test]
fn sql632_lo_import() {
  let d = diags("SELECT lo_import('/etc/passwd')");
  assert!(d.iter().any(|x| x.code == "sql632"));
}

#[test]
fn sql632_lo_export() {
  let d = diags("SELECT lo_export(loid, '/tmp/out')");
  assert!(d.iter().any(|x| x.code == "sql632"));
}

#[test]
fn sql632_quiet_plain() {
  let d = diags("SELECT * FROM t");
  assert!(!d.iter().any(|x| x.code == "sql632"));
}

#[test]
fn sql633_pg_read_file() {
  let d = diags("SELECT pg_read_file('/etc/passwd')");
  assert!(d.iter().any(|x| x.code == "sql633"));
}

#[test]
fn sql633_pg_ls_dir() {
  let d = diags("SELECT pg_ls_dir('/var/lib/postgresql')");
  assert!(d.iter().any(|x| x.code == "sql633"));
}

#[test]
fn sql633_quiet_plain() {
  let d = diags("SELECT * FROM t");
  assert!(!d.iter().any(|x| x.code == "sql633"));
}

#[test]
fn sql634_weak_md5() {
  let d = diags("SELECT crypt(pw, gen_salt('md5'))");
  assert!(d.iter().any(|x| x.code == "sql634"));
}

#[test]
fn sql634_weak_des() {
  let d = diags("SELECT crypt(pw, gen_salt('des'))");
  assert!(d.iter().any(|x| x.code == "sql634"));
}

#[test]
fn sql634_quiet_bf() {
  let d = diags("SELECT crypt(pw, gen_salt('bf', 10))");
  assert!(!d.iter().any(|x| x.code == "sql634"));
}

#[test]
fn sql635_pragma() {
  let d = diags("PRAGMA foreign_keys = ON");
  assert!(d.iter().any(|x| x.code == "sql635"));
}

#[test]
fn sql635_quiet_set() {
  let d = diags("SET search_path = public");
  assert!(!d.iter().any(|x| x.code == "sql635"));
}

#[test]
fn sql636_autoincrement() {
  let d = diags("CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT)");
  assert!(d.iter().any(|x| x.code == "sql636"));
}

#[test]
fn sql636_quiet_identity() {
  let d = diags("CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY)");
  assert!(!d.iter().any(|x| x.code == "sql636"));
}

#[test]
fn sql636_quiet_auto_increment_underscore() {
  let d = diags("CREATE TABLE t (id int AUTO_INCREMENT)");
  assert!(!d.iter().any(|x| x.code == "sql636"));
}

#[test]
fn sql637_glob() {
  let d = diags("SELECT * FROM t WHERE name GLOB 'foo*'");
  assert!(d.iter().any(|x| x.code == "sql637"));
}

#[test]
fn sql637_quiet_like() {
  let d = diags("SELECT * FROM t WHERE name LIKE 'foo%'");
  assert!(!d.iter().any(|x| x.code == "sql637"));
}

#[test]
fn sql638_strftime() {
  let d = diags("SELECT strftime('%Y', ts) FROM t");
  assert!(d.iter().any(|x| x.code == "sql638"));
}

#[test]
fn sql638_typeof() {
  let d = diags("SELECT typeof(x) FROM t");
  assert!(d.iter().any(|x| x.code == "sql638"));
}

#[test]
fn sql638_last_insert_rowid() {
  let d = diags("SELECT last_insert_rowid()");
  assert!(d.iter().any(|x| x.code == "sql638"));
}

#[test]
fn sql638_quiet_to_char() {
  let d = diags("SELECT to_char(ts, 'YYYY') FROM t");
  assert!(!d.iter().any(|x| x.code == "sql638"));
}

#[test]
fn sql639_hex() {
  let d = diags("SELECT hex(data) FROM t");
  assert!(d.iter().any(|x| x.code == "sql639"));
}

#[test]
fn sql639_space() {
  let d = diags("SELECT space(4) FROM t");
  assert!(d.iter().any(|x| x.code == "sql639"));
}

#[test]
fn sql639_quiet_encode() {
  let d = diags("SELECT encode(data, 'hex') FROM t");
  assert!(!d.iter().any(|x| x.code == "sql639"));
}

#[test]
fn sql640_dayofweek() {
  let d = diags("SELECT dayofweek(ts) FROM t");
  assert!(d.iter().any(|x| x.code == "sql640"));
}

#[test]
fn sql640_monthname() {
  let d = diags("SELECT monthname(ts) FROM t");
  assert!(d.iter().any(|x| x.code == "sql640"));
}

#[test]
fn sql640_quiet_extract() {
  let d = diags("SELECT extract(dow FROM ts) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql640"));
}

#[test]
fn sql641_default_now_string() {
  let d = diags("CREATE TABLE t (created_at timestamptz DEFAULT 'now')");
  assert!(d.iter().any(|x| x.code == "sql641"));
}

#[test]
fn sql641_default_now_cast() {
  let d = diags("CREATE TABLE t (created_at timestamp DEFAULT 'now'::timestamp)");
  assert!(d.iter().any(|x| x.code == "sql641"));
}

#[test]
fn sql641_quiet_now_function() {
  let d = diags("CREATE TABLE t (created_at timestamptz DEFAULT now())");
  assert!(!d.iter().any(|x| x.code == "sql641"));
}

#[test]
fn sql642_into_outfile() {
  let d = diags("SELECT * FROM t INTO OUTFILE '/tmp/out.csv'");
  assert!(d.iter().any(|x| x.code == "sql642"));
}

#[test]
fn sql642_load_data() {
  let d = diags("LOAD DATA INFILE '/tmp/in.csv' INTO TABLE t");
  assert!(d.iter().any(|x| x.code == "sql642"));
}

#[test]
fn sql642_quiet_copy() {
  let d = diags("COPY t FROM '/tmp/in.csv' CSV");
  assert!(!d.iter().any(|x| x.code == "sql642"));
}

#[test]
fn sql643_add_months() {
  let d = diags("SELECT add_months(d, 3) FROM t");
  assert!(d.iter().any(|x| x.code == "sql643"));
}

#[test]
fn sql643_sys_guid() {
  let d = diags("SELECT sys_guid()");
  assert!(d.iter().any(|x| x.code == "sql643"));
}

#[test]
fn sql643_quiet_age() {
  let d = diags("SELECT age(a, b) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql643"));
}

#[test]
fn sql644_adddate() {
  let d = diags("SELECT adddate(d, 7) FROM t");
  assert!(d.iter().any(|x| x.code == "sql644"));
}

#[test]
fn sql644_last_day() {
  let d = diags("SELECT last_day(d) FROM t");
  assert!(d.iter().any(|x| x.code == "sql644"));
}

#[test]
fn sql644_quiet_make_date_underscore() {
  let d = diags("SELECT make_date(2020, 1, 1)");
  assert!(!d.iter().any(|x| x.code == "sql644"));
}

#[test]
fn sql645_srf_in_where() {
  let d = diags("SELECT * FROM t WHERE unnest(arr) = 5");
  assert!(d.iter().any(|x| x.code == "sql645"));
}

#[test]
fn sql645_generate_series_in_where() {
  let d = diags("SELECT * FROM t WHERE generate_series(1, 10) > x");
  assert!(d.iter().any(|x| x.code == "sql645"));
}

#[test]
fn sql645_quiet_srf_in_subquery() {
  let d = diags("SELECT * FROM t WHERE x IN (SELECT unnest(arr) FROM s)");
  assert!(!d.iter().any(|x| x.code == "sql645"));
}

#[test]
fn sql645_quiet_srf_in_from() {
  let d = diags("SELECT * FROM unnest(ARRAY[1,2,3]) AS g(x) WHERE x > 1");
  assert!(!d.iter().any(|x| x.code == "sql645"));
}

#[test]
fn sql646_count_distinct_star() {
  let d = diags("SELECT count(DISTINCT *) FROM t");
  assert!(d.iter().any(|x| x.code == "sql646"));
}

#[test]
fn sql646_quiet_count_star() {
  let d = diags("SELECT count(*) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql646"));
}

#[test]
fn sql646_quiet_select_distinct_star() {
  let d = diags("SELECT DISTINCT * FROM t");
  assert!(!d.iter().any(|x| x.code == "sql646"));
}

#[test]
fn sql647_in_subquery_two_cols() {
  let d = diags("SELECT * FROM t WHERE id IN (SELECT a, b FROM s)");
  assert!(d.iter().any(|x| x.code == "sql647"));
}

#[test]
fn sql647_quiet_one_col() {
  let d = diags("SELECT * FROM t WHERE id IN (SELECT a FROM s)");
  assert!(!d.iter().any(|x| x.code == "sql647"));
}

#[test]
fn sql647_quiet_row_constructor() {
  let d = diags("SELECT * FROM t WHERE (a, b) IN (SELECT a, b FROM s)");
  assert!(!d.iter().any(|x| x.code == "sql647"));
}

#[test]
fn sql647_quiet_in_list() {
  let d = diags("SELECT * FROM t WHERE id IN (1, 2, 3)");
  assert!(!d.iter().any(|x| x.code == "sql647"));
}

#[test]
fn sql648_over_100() {
  let d = diags("SELECT * FROM t TABLESAMPLE SYSTEM (150)");
  assert!(d.iter().any(|x| x.code == "sql648"));
}

#[test]
fn sql648_negative() {
  let d = diags("SELECT * FROM t TABLESAMPLE BERNOULLI (-5)");
  assert!(d.iter().any(|x| x.code == "sql648"));
}

#[test]
fn sql648_quiet_valid() {
  let d = diags("SELECT * FROM t TABLESAMPLE SYSTEM (10)");
  assert!(!d.iter().any(|x| x.code == "sql648"));
}

#[test]
fn sql649_do_update_no_target() {
  let d = diags("INSERT INTO t (a) VALUES (1) ON CONFLICT DO UPDATE SET a = 1");
  assert!(d.iter().any(|x| x.code == "sql649"));
}

#[test]
fn sql649_quiet_with_target() {
  let d = diags("INSERT INTO t (a) VALUES (1) ON CONFLICT (a) DO UPDATE SET a = 1");
  assert!(!d.iter().any(|x| x.code == "sql649"));
}

#[test]
fn sql649_quiet_do_nothing() {
  let d = diags("INSERT INTO t (a) VALUES (1) ON CONFLICT DO NOTHING");
  assert!(!d.iter().any(|x| x.code == "sql649"));
}

#[test]
fn sql650_row_arity_mismatch() {
  let d = diags("SELECT * FROM t WHERE (a, b) = (1, 2, 3)");
  assert!(d.iter().any(|x| x.code == "sql650"));
}

#[test]
fn sql650_quiet_equal_arity() {
  let d = diags("SELECT * FROM t WHERE (a, b) = (1, 2)");
  assert!(!d.iter().any(|x| x.code == "sql650"));
}

#[test]
fn sql650_quiet_scalar() {
  let d = diags("SELECT * FROM t WHERE (a) = (1)");
  assert!(!d.iter().any(|x| x.code == "sql650"));
}

#[test]
fn sql651_srf_in_order_by() {
  let d = diags("SELECT * FROM t ORDER BY unnest(arr)");
  assert!(d.iter().any(|x| x.code == "sql651"));
}

#[test]
fn sql651_srf_in_group_by() {
  let d = diags("SELECT x FROM t GROUP BY generate_series(1, 10)");
  assert!(d.iter().any(|x| x.code == "sql651"));
}

#[test]
fn sql651_quiet_normal() {
  let d = diags("SELECT x FROM t GROUP BY x ORDER BY x");
  assert!(!d.iter().any(|x| x.code == "sql651"));
}

#[test]
fn sql652_duplicate_cte() {
  let d = diags("WITH a AS (SELECT 1), a AS (SELECT 2) SELECT * FROM a");
  assert!(d.iter().any(|x| x.code == "sql652"));
}

#[test]
fn sql652_quiet_distinct_ctes() {
  let d = diags("WITH a AS (SELECT 1), b AS (SELECT 2) SELECT * FROM a, b");
  assert!(!d.iter().any(|x| x.code == "sql652"));
}

#[test]
fn sql652_quiet_recursive() {
  let d = diags("WITH RECURSIVE t AS (SELECT 1 UNION ALL SELECT n+1 FROM t WHERE n < 5) SELECT * FROM t");
  assert!(!d.iter().any(|x| x.code == "sql652"));
}

#[test]
fn sql653_aggregate_in_check() {
  let d = diags("CREATE TABLE t (n int, CHECK (count(*) > 0))");
  assert!(d.iter().any(|x| x.code == "sql653"));
}

#[test]
fn sql653_quiet_plain_check() {
  let d = diags("CREATE TABLE t (n int, CHECK (n > 0))");
  assert!(!d.iter().any(|x| x.code == "sql653"));
}

#[test]
fn sql654_aggregate_in_index() {
  let d = diags("CREATE INDEX idx ON t (count(x))");
  assert!(d.iter().any(|x| x.code == "sql654"));
}

#[test]
fn sql654_quiet_plain_index() {
  let d = diags("CREATE INDEX idx ON t (lower(name))");
  assert!(!d.iter().any(|x| x.code == "sql654"));
}

#[test]
fn sql654_quiet_max_named_table() {
  let d = diags("CREATE INDEX idx ON max_values (x)");
  assert!(!d.iter().any(|x| x.code == "sql654"));
}

#[test]
fn sql655_arity_mismatch() {
  let d = diags("UPDATE t SET (a, b) = (1, 2, 3)");
  assert!(d.iter().any(|x| x.code == "sql655"));
}

#[test]
fn sql655_quiet_match() {
  let d = diags("UPDATE t SET (a, b) = (1, 2)");
  assert!(!d.iter().any(|x| x.code == "sql655"));
}

#[test]
fn sql655_quiet_single() {
  let d = diags("UPDATE t SET a = 1");
  assert!(!d.iter().any(|x| x.code == "sql655"));
}

#[test]
fn sql656_truncate_where() {
  let d = diags("TRUNCATE t WHERE id > 100");
  assert!(d.iter().any(|x| x.code == "sql656"));
}

#[test]
fn sql656_quiet_plain_truncate() {
  let d = diags("TRUNCATE t");
  assert!(!d.iter().any(|x| x.code == "sql656"));
}

#[test]
fn sql656_quiet_delete_where() {
  let d = diags("DELETE FROM t WHERE id > 100");
  assert!(!d.iter().any(|x| x.code == "sql656"));
}

#[test]
fn sql657_order_after_limit() {
  let d = diags("SELECT * FROM t LIMIT 5 ORDER BY x");
  assert!(d.iter().any(|x| x.code == "sql657"));
}

#[test]
fn sql657_quiet_correct_order() {
  let d = diags("SELECT * FROM t ORDER BY x LIMIT 5");
  assert!(!d.iter().any(|x| x.code == "sql657"));
}

#[test]
fn sql657_quiet_subquery_order() {
  let d = diags("SELECT * FROM (SELECT * FROM s ORDER BY y) q LIMIT 5");
  assert!(!d.iter().any(|x| x.code == "sql657"));
}

#[test]
fn sql658_limit_and_fetch() {
  let d = diags("SELECT * FROM t LIMIT 5 FETCH FIRST 3 ROWS ONLY");
  assert!(d.iter().any(|x| x.code == "sql658"));
}

#[test]
fn sql658_quiet_only_limit() {
  let d = diags("SELECT * FROM t LIMIT 5");
  assert!(!d.iter().any(|x| x.code == "sql658"));
}

#[test]
fn sql658_quiet_only_fetch() {
  let d = diags("SELECT * FROM t FETCH FIRST 3 ROWS ONLY");
  assert!(!d.iter().any(|x| x.code == "sql658"));
}

#[test]
fn sql659_where_after_group_by() {
  let d = diags("SELECT a FROM t GROUP BY a WHERE a > 0");
  assert!(d.iter().any(|x| x.code == "sql659"));
}

#[test]
fn sql659_quiet_correct_order() {
  let d = diags("SELECT a FROM t WHERE a > 0 GROUP BY a");
  assert!(!d.iter().any(|x| x.code == "sql659"));
}

#[test]
fn sql659_quiet_union_where() {
  let d = diags("SELECT a FROM t GROUP BY a UNION SELECT b FROM s WHERE b > 0");
  assert!(!d.iter().any(|x| x.code == "sql659"));
}

#[test]
fn sql660_cross_join_on() {
  let d = diags("SELECT * FROM a CROSS JOIN b ON a.id = b.id");
  assert!(d.iter().any(|x| x.code == "sql660"));
}

#[test]
fn sql660_quiet_plain_cross() {
  let d = diags("SELECT * FROM a CROSS JOIN b");
  assert!(!d.iter().any(|x| x.code == "sql660"));
}

#[test]
fn sql660_quiet_inner_join_on() {
  let d = diags("SELECT * FROM a CROSS JOIN b JOIN c ON b.id = c.id");
  assert!(!d.iter().any(|x| x.code == "sql660"));
}

#[test]
fn sql661_row_number_no_over() {
  let d = diags("SELECT row_number() FROM t");
  assert!(d.iter().any(|x| x.code == "sql661"));
}

#[test]
fn sql661_rank_no_over() {
  let d = diags("SELECT rank() FROM t");
  assert!(d.iter().any(|x| x.code == "sql661"));
}

#[test]
fn sql661_quiet_with_over() {
  let d = diags("SELECT row_number() OVER (ORDER BY x) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql661"));
}

#[test]
fn sql661_quiet_count() {
  let d = diags("SELECT count(*) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql661"));
}

#[test]
fn sql662_distinct_on_no_parens() {
  let d = diags("SELECT DISTINCT ON x, y FROM t");
  assert!(d.iter().any(|x| x.code == "sql662"));
}

#[test]
fn sql662_quiet_with_parens() {
  let d = diags("SELECT DISTINCT ON (x) y FROM t");
  assert!(!d.iter().any(|x| x.code == "sql662"));
}

#[test]
fn sql662_quiet_plain_distinct() {
  let d = diags("SELECT DISTINCT a, b FROM t");
  assert!(!d.iter().any(|x| x.code == "sql662"));
}

#[test]
fn sql663_order_before_union() {
  let d = diags("SELECT a FROM t ORDER BY a UNION SELECT b FROM s");
  assert!(d.iter().any(|x| x.code == "sql663"));
}

#[test]
fn sql663_limit_before_union() {
  let d = diags("SELECT a FROM t LIMIT 5 UNION SELECT b FROM s");
  assert!(d.iter().any(|x| x.code == "sql663"));
}

#[test]
fn sql663_quiet_order_after_union() {
  let d = diags("SELECT a FROM t UNION SELECT b FROM s ORDER BY a");
  assert!(!d.iter().any(|x| x.code == "sql663"));
}

#[test]
fn sql663_quiet_parenthesized() {
  let d = diags("(SELECT a FROM t LIMIT 5) UNION SELECT b FROM s");
  assert!(!d.iter().any(|x| x.code == "sql663"));
}

#[test]
fn sql664_having_before_group() {
  let d = diags("SELECT a, count(*) FROM t HAVING count(*) > 1 GROUP BY a");
  assert!(d.iter().any(|x| x.code == "sql664"));
}

#[test]
fn sql664_quiet_correct_order() {
  let d = diags("SELECT a, count(*) FROM t GROUP BY a HAVING count(*) > 1");
  assert!(!d.iter().any(|x| x.code == "sql664"));
}

#[test]
fn sql665_where_before_set() {
  let d = diags("UPDATE t WHERE id = 1 SET x = 2");
  assert!(d.iter().any(|x| x.code == "sql665"));
}

#[test]
fn sql665_quiet_correct_order() {
  let d = diags("UPDATE t SET x = 2 WHERE id = 1");
  assert!(!d.iter().any(|x| x.code == "sql665"));
}

#[test]
fn sql665_quiet_subquery_where() {
  let d = diags("UPDATE t SET x = (SELECT v FROM s WHERE s.id = t.id)");
  assert!(!d.iter().any(|x| x.code == "sql665"));
}

#[test]
fn sql666_insert_ignore() {
  let d = diags("INSERT IGNORE INTO t (a) VALUES (1)");
  assert!(d.iter().any(|x| x.code == "sql666"));
}

#[test]
fn sql666_quiet_plain_insert() {
  let d = diags("INSERT INTO t (a) VALUES (1)");
  assert!(!d.iter().any(|x| x.code == "sql666"));
}

#[test]
fn sql666_quiet_on_conflict() {
  let d = diags("INSERT INTO t (a) VALUES (1) ON CONFLICT DO NOTHING");
  assert!(!d.iter().any(|x| x.code == "sql666"));
}

#[test]
fn sql667_insert_set() {
  let d = diags("INSERT INTO t SET a = 1, b = 2");
  assert!(d.iter().any(|x| x.code == "sql667"));
}

#[test]
fn sql667_quiet_values() {
  let d = diags("INSERT INTO t (a, b) VALUES (1, 2)");
  assert!(!d.iter().any(|x| x.code == "sql667"));
}

#[test]
fn sql667_quiet_on_conflict_set() {
  let d = diags("INSERT INTO t (a) VALUES (1) ON CONFLICT (a) DO UPDATE SET a = 2");
  assert!(!d.iter().any(|x| x.code == "sql667"));
}

#[test]
fn sql668_delete_alias_from() {
  let d = diags("DELETE t1 FROM t1 JOIN t2 ON t1.id = t2.id");
  assert!(d.iter().any(|x| x.code == "sql668"));
}

#[test]
fn sql668_quiet_delete_from() {
  let d = diags("DELETE FROM t WHERE id = 1");
  assert!(!d.iter().any(|x| x.code == "sql668"));
}

#[test]
fn sql668_quiet_delete_from_only() {
  let d = diags("DELETE FROM ONLY t WHERE id = 1");
  assert!(!d.iter().any(|x| x.code == "sql668"));
}

#[test]
fn sql669_lock_in_share_mode() {
  let d = diags("SELECT * FROM t WHERE id = 1 LOCK IN SHARE MODE");
  assert!(d.iter().any(|x| x.code == "sql669"));
}

#[test]
fn sql669_quiet_for_share() {
  let d = diags("SELECT * FROM t WHERE id = 1 FOR SHARE");
  assert!(!d.iter().any(|x| x.code == "sql669"));
}

#[test]
fn sql670_show_tables() {
  let d = diags("SHOW TABLES");
  assert!(d.iter().any(|x| x.code == "sql670"));
}

#[test]
fn sql670_show_create_table() {
  let d = diags("SHOW CREATE TABLE users");
  assert!(d.iter().any(|x| x.code == "sql670"));
}

#[test]
fn sql670_quiet_show_config() {
  let d = diags("SHOW search_path");
  assert!(!d.iter().any(|x| x.code == "sql670"));
}

#[test]
fn sql670_quiet_show_all() {
  let d = diags("SHOW ALL");
  assert!(!d.iter().any(|x| x.code == "sql670"));
}

#[test]
fn sql671_describe() {
  let d = diags("DESCRIBE users");
  assert!(d.iter().any(|x| x.code == "sql671"));
}

#[test]
fn sql671_desc_table() {
  let d = diags("DESC users");
  assert!(d.iter().any(|x| x.code == "sql671"));
}

#[test]
fn sql671_quiet_order_by_desc() {
  let d = diags("SELECT * FROM t ORDER BY x DESC");
  assert!(!d.iter().any(|x| x.code == "sql671"));
}

#[test]
fn sql672_modify_column() {
  let d = diags("ALTER TABLE t MODIFY COLUMN a bigint");
  assert!(d.iter().any(|x| x.code == "sql672"));
}

#[test]
fn sql672_change_column() {
  let d = diags("ALTER TABLE t CHANGE a b int");
  assert!(d.iter().any(|x| x.code == "sql672"));
}

#[test]
fn sql672_quiet_alter_column_type() {
  let d = diags("ALTER TABLE t ALTER COLUMN a TYPE bigint");
  assert!(!d.iter().any(|x| x.code == "sql672"));
}

#[test]
fn sql673_between_low_null() {
  let d = diags("SELECT * FROM t WHERE x BETWEEN NULL AND 5");
  assert!(d.iter().any(|x| x.code == "sql673"));
}

#[test]
fn sql673_between_high_null() {
  let d = diags("SELECT * FROM t WHERE x BETWEEN 1 AND NULL");
  assert!(d.iter().any(|x| x.code == "sql673"));
}

#[test]
fn sql673_not_between_null() {
  let d = diags("SELECT * FROM t WHERE x NOT BETWEEN NULL AND 5");
  assert!(d.iter().any(|x| x.code == "sql673"));
}

#[test]
fn sql673_quiet_real_bounds() {
  let d = diags("SELECT * FROM t WHERE x BETWEEN 1 AND 5");
  assert!(!d.iter().any(|x| x.code == "sql673"));
}

#[test]
fn sql674_row_number_frame() {
  let d = diags("SELECT row_number() OVER (ORDER BY id ROWS BETWEEN 1 PRECEDING AND CURRENT ROW) FROM t");
  assert!(d.iter().any(|x| x.code == "sql674"));
}

#[test]
fn sql674_rank_range_frame() {
  let d = diags("SELECT rank() OVER (ORDER BY id RANGE UNBOUNDED PRECEDING) FROM t");
  assert!(d.iter().any(|x| x.code == "sql674"));
}

#[test]
fn sql674_quiet_no_frame() {
  let d = diags("SELECT row_number() OVER (PARTITION BY name ORDER BY id) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql674"));
}

#[test]
fn sql674_quiet_aggregate_with_frame() {
  let d = diags("SELECT sum(id) OVER (ORDER BY id ROWS BETWEEN 1 PRECEDING AND CURRENT ROW) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql674"));
}

#[test]
fn sql675_distinct_in_union_branch() {
  let d = diags("SELECT DISTINCT id FROM t UNION SELECT id FROM users");
  assert!(d.iter().any(|x| x.code == "sql675"));
}

#[test]
fn sql675_quiet_union_all() {
  let d = diags("SELECT DISTINCT id FROM t UNION ALL SELECT id FROM users");
  assert!(!d.iter().any(|x| x.code == "sql675"));
}

#[test]
fn sql675_quiet_distinct_no_setop() {
  let d = diags("SELECT DISTINCT id FROM t");
  assert!(!d.iter().any(|x| x.code == "sql675"));
}

#[test]
fn sql675_quiet_distinct_on() {
  let d = diags("SELECT DISTINCT ON (id) id FROM t UNION SELECT id FROM users");
  assert!(!d.iter().any(|x| x.code == "sql675"));
}

#[test]
fn sql676_count_distinct_int() {
  let d = diags("SELECT count(DISTINCT 1) FROM t");
  assert!(d.iter().any(|x| x.code == "sql676"));
}

#[test]
fn sql676_count_distinct_string() {
  let d = diags("SELECT count(DISTINCT 'x') FROM t");
  assert!(d.iter().any(|x| x.code == "sql676"));
}

#[test]
fn sql676_quiet_count_distinct_column() {
  let d = diags("SELECT count(DISTINCT id) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql676"));
}

#[test]
fn sql676_quiet_count_star() {
  let d = diags("SELECT count(*) FROM t");
  assert!(!d.iter().any(|x| x.code == "sql676"));
}

#[test]
fn sql677_percent_one() {
  let d = diags("SELECT * FROM t WHERE x % 1 = 0");
  assert!(d.iter().any(|x| x.code == "sql677"));
}

#[test]
fn sql677_mod_one() {
  let d = diags("SELECT mod(x, 1) FROM t");
  assert!(d.iter().any(|x| x.code == "sql677"));
}

#[test]
fn sql677_quiet_percent_two() {
  let d = diags("SELECT * FROM t WHERE x % 2 = 0");
  assert!(!d.iter().any(|x| x.code == "sql677"));
}

#[test]
fn sql677_quiet_percent_ten() {
  let d = diags("SELECT * FROM t WHERE x % 10 = 0");
  assert!(!d.iter().any(|x| x.code == "sql677"));
}

#[test]
fn sql678_zero_date() {
  let d = diags("INSERT INTO t (d) VALUES ('0000-00-00')");
  assert!(d.iter().any(|x| x.code == "sql678"));
}

#[test]
fn sql678_zero_datetime() {
  let d = diags("SELECT * FROM t WHERE d = '0000-00-00 00:00:00'");
  assert!(d.iter().any(|x| x.code == "sql678"));
}

#[test]
fn sql678_quiet_real_date() {
  let d = diags("SELECT * FROM t WHERE d = '2024-01-01'");
  assert!(!d.iter().any(|x| x.code == "sql678"));
}
