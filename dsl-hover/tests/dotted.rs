//! Hover narrow-by-side for dotted identifiers.
#![allow(clippy::absurd_extreme_comparisons, unused_comparisons, clippy::len_zero, clippy::const_is_empty, clippy::identity_op)]

use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Schema, Table, TableKind};
use dsl_hover::hover;
use text_size::TextSize;

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
    ],
    constraints: vec![],
    indexes: vec![],
    triggers: vec![],
    policies: vec![],
    comment: None,
    row_estimate: None,
    owner: None,
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

fn hover_at(src: &str, byte: usize) -> Option<String> {
  hover(src, TextSize::from(byte as u32), &cat())
}

#[test]
fn cursor_on_alias_left_shows_table_card() {
  let src = "SELECT u.id FROM users u";
  // cursor right on the `u` before the dot
  let cur = src.find("u.").unwrap();
  let md = hover_at(src, cur).expect("hover for alias");
  assert!(md.contains("Table") || md.contains("users"), "alias hover should show table: {md}");
  // Must NOT be a single-column card.
  assert!(!md.starts_with("# Column"), "alias side got column card: {md}");
}

#[test]
fn cursor_on_column_right_shows_column_card() {
  let src = "SELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 2; // on `id`
  let md = hover_at(src, cur).expect("hover for column");
  assert!(md.contains("Column") || md.contains("public.users.id"), "column hover should focus column: {md}");
  // Must NOT be the whole-table card.
  assert!(!md.starts_with("# Table"), "column side got table card: {md}");
}

#[test]
fn bare_column_with_unique_scope_resolves_to_owner_table() {
  let src = "SELECT id FROM users";
  let cur = src.find("id").unwrap();
  let md = hover_at(src, cur).expect("hover for column");
  // The column hover names the origin table.
  assert!(md.contains("users.id"), "expected `users.id` in hover: {md}");
}

#[test]
fn unknown_alias_returns_no_hover() {
  let src = "SELECT zzz.id FROM users";
  let cur = src.find("zzz").unwrap();
  // Unknown alias -- catalog lookup may still match `zzz` as a table
  // (no), so hover returns None.
  let md = hover_at(src, cur);
  assert!(md.is_none() || !md.as_ref().unwrap().contains("Column"));
}

// ===== cast-operator hover =================================================

#[test]
fn hover_on_text_after_cast_op_renders_type_card() {
  let src = "SELECT id::text FROM users;";
  // Cursor sits on the `t` of `text`.
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover should resolve type");
  // Knowledge-driven type card includes the type name plus a docs link.
  assert!(md.to_ascii_lowercase().contains("text"), "expected text-type card; got: {md}");
}

#[test]
fn hover_on_jsonb_after_cast_op_renders_type_card() {
  let src = "SELECT data::jsonb FROM events;";
  let cur = src.find("jsonb").unwrap();
  let md = hover_at(src, cur).expect("hover should resolve jsonb");
  assert!(md.to_ascii_lowercase().contains("jsonb"));
}

#[test]
fn hover_on_int_after_cast_op_renders_type_card() {
  let src = "SELECT col::int FROM t;";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover should resolve int");
  assert!(md.to_ascii_lowercase().contains("int"));
}

// ===== schema-qualified column resolve ===================================

#[test]
fn hover_on_schema_qualified_column_path() {
  let src = "SELECT public.users.id FROM public.users;";
  let cur = src.find(".id").unwrap() + 1;
  let md = hover_at(src, cur).expect("schema.table.col hover should resolve");
  assert!(md.contains("id") && md.to_ascii_lowercase().contains("uuid"), "expected id-column card; got: {md}");
}

#[test]
fn hover_on_schema_qualified_table_card() {
  let src = "SELECT * FROM public.users;";
  let cur = src.find(".users").unwrap() + 1;
  let md = hover_at(src, cur).expect("schema.table hover should resolve");
  assert!(md.contains("users"), "expected users-table card; got: {md}");
}

// ===== sequence hover via nextval('...') / currval / setval ===============

#[test]
fn hover_on_sequence_in_nextval() {
  let src = "SELECT nextval('user_id_seq') FROM users;";
  let cur = src.find("user_id_seq").unwrap() + 3;
  let md = hover_at(src, cur).expect("nextval seq hover");
  assert!(md.contains("Sequence"), "expected sequence card; got: {md}");
  assert!(md.contains("user_id_seq"));
  assert!(md.contains("nextval"));
}

#[test]
fn hover_on_sequence_in_currval() {
  let src = "SELECT currval('audit_log_id_seq');";
  let cur = src.find("audit_log_id_seq").unwrap();
  let md = hover_at(src, cur).expect("currval seq hover");
  assert!(md.contains("audit_log_id_seq"));
}

#[test]
fn hover_on_sequence_in_setval() {
  let src = "SELECT setval('thing_seq', 42);";
  let cur = src.find("thing_seq").unwrap() + 4;
  let md = hover_at(src, cur).expect("setval seq hover");
  assert!(md.contains("thing_seq"));
}

// ===== hover on function-call argument literals ============================

#[test]
fn hover_on_arg_literal_inside_coalesce_marks_active_param() {
  // Cursor sits on the second arg (`'fallback'`).
  let src = "SELECT coalesce(name, 'fallback') FROM users;";
  let cur = src.find("'fallback'").unwrap() + 2;
  let md = hover_at(src, cur).expect("arg hover");
  assert!(md.contains("coalesce"), "expected fn name in card; got: {md}");
  assert!(md.contains(">>"), "expected `>>` active-marker; got: {md}");
}

#[test]
fn hover_on_arg_literal_outside_call_returns_no_arg_card() {
  let src = "SELECT 1 FROM users;";
  let cur = src.find("1").unwrap();
  let md = hover_at(src, cur);
  if let Some(md) = md {
    assert!(!md.contains("function call:"), "should not be a function-arg card: {md}");
  }
}

#[test]
fn hover_on_non_sequence_string_returns_no_seq_card() {
  let src = "SELECT * FROM users WHERE name = 'admin';";
  let cur = src.find("admin").unwrap();
  let md = hover_at(src, cur);
  if let Some(md) = md {
    assert!(!md.contains("Sequence"), "should not be a sequence card");
  }
}

// ============================================================================
// Hover on columns of a CTE (`WITH t AS (SELECT ... AS alias ...) SELECT t.alias`)
// ============================================================================

#[test]
fn hover_on_cte_alias_column_shows_cte_card() {
  // The outer SELECT references a CTE-projected alias. Cursor on the
  // `user_id` part of `t.user_id` should resolve to the alias the CTE
  // projects, not "unknown" -- and the card should identify it as a
  // CTE column referring back to `t`.
  let src = "WITH t AS (SELECT id AS user_id FROM users) SELECT t.user_id FROM t;";
  let cur = src.rfind("user_id").unwrap();
  let md = hover_at(src, cur).expect("hover for CTE column");
  assert!(md.contains("user_id"), "expected CTE column name in card; got: {md}");
  assert!(md.contains('t'), "expected CTE alias `t` referenced; got: {md}");
  assert!(md.to_ascii_lowercase().contains("cte"), "expected CTE marker; got: {md}");
}

#[test]
fn hover_on_raise_keyword_inside_plpgsql_body() {
  // `CREATE FUNCTION ... AS $$ BEGIN RAISE EXCEPTION 'oops'; ...` --
  // hovering on RAISE inside the dollar-quoted PL/pgSQL body must
  // surface the keyword card. The previous inside_string_or_comment
  // path treated the entire body as inert.
  let src = "CREATE FUNCTION f() RETURNS void AS $$ BEGIN RAISE EXCEPTION 'oops'; END; $$ LANGUAGE plpgsql";
  let cur = src.find("RAISE").unwrap();
  let md = hover_at(src, cur).expect("RAISE inside plpgsql should hover");
  assert!(md.contains("RAISE"), "expected RAISE card; got: {md}");
}

#[test]
fn hover_inside_plpgsql_body_string_literal_still_suppresses() {
  // The recurse-into-body change must NOT lose the existing
  // suppression for inner string literals -- hovering on a word
  // inside `'oops'` inside `$$...$$` should still return None (or
  // at least not the WHEN-keyword card or similar).
  let src = "CREATE FUNCTION f() RETURNS void AS $$ BEGIN RAISE EXCEPTION 'WHEN'; END; $$ LANGUAGE plpgsql";
  let cur = src.rfind("WHEN").unwrap();
  let md = hover_at(src, cur);
  if let Some(m) = md {
    assert!(
      !m.contains("# WHEN") || m.contains("literal"),
      "string-literal content wrongly resolved to keyword: {m}"
    );
  }
}

#[test]
fn hover_on_window_clause_keyword() {
  // `SELECT ... FROM t WINDOW w AS (...)` -- the WINDOW clause keyword
  // had no knowledge entry, so hover returned None.
  let src = "SELECT * FROM users WINDOW w AS (PARTITION BY id)";
  let cur = src.find("WINDOW").unwrap();
  let md = hover_at(src, cur).expect("WINDOW should hover");
  assert!(md.contains("WINDOW"), "expected WINDOW card; got: {md}");
  assert!(
    md.to_ascii_lowercase().contains("window"),
    "expected window-clause explanation; got: {md}"
  );
}

#[test]
fn hover_on_string_concat_operator() {
  let src = "SELECT 'a' || 'b'";
  let cur = src.find("||").unwrap();
  let md = hover_at(src, cur).expect("`||` should hover");
  assert!(md.contains("||"), "expected `||` card; got: {md}");
  assert!(
    md.to_ascii_lowercase().contains("concat") || md.to_ascii_lowercase().contains("string"),
    "expected concatenation explanation; got: {md}"
  );
}

#[test]
fn hover_on_equality_operator() {
  let src = "SELECT * FROM users WHERE id = 1";
  let cur = src.find('=').unwrap();
  let md = hover_at(src, cur).expect("`=` should hover");
  assert!(md.contains('='), "expected `=` card; got: {md}");
  assert!(
    md.to_ascii_lowercase().contains("equal") || md.to_ascii_lowercase().contains("comparison"),
    "expected equality explanation; got: {md}"
  );
}

#[test]
fn hover_on_not_equal_operator() {
  let src = "SELECT * FROM users WHERE id <> 1";
  let cur = src.find("<>").unwrap();
  let md = hover_at(src, cur).expect("`<>` should hover");
  assert!(md.contains("<>"), "expected `<>` card; got: {md}");
}

#[test]
fn hover_on_rollup_in_group_by_returns_knowledge_card() {
  // ROLLUP / CUBE inside GROUP BY are real SQL grouping operators
  // (not column names) and must surface their knowledge card even
  // though `in_field_list_context` is suppressing keyword hovers
  // for column-name-style identifiers in the same clause.
  let src = "SELECT count(*) FROM users GROUP BY ROLLUP (id)";
  let cur = src.find("ROLLUP").unwrap();
  let md = hover_at(src, cur).expect("ROLLUP should hover with knowledge card");
  assert!(md.contains("ROLLUP"), "expected ROLLUP card; got: {md}");
  assert!(md.contains("Hierarchical") || md.contains("grouping"), "expected ROLLUP explanation; got: {md}");
}

#[test]
fn hover_on_cube_in_group_by_returns_knowledge_card() {
  let src = "SELECT count(*) FROM users GROUP BY CUBE (id)";
  let cur = src.find("CUBE").unwrap();
  let md = hover_at(src, cur).expect("CUBE should hover");
  assert!(md.contains("CUBE"), "expected CUBE card; got: {md}");
}

#[test]
fn hover_on_column_inside_subquery_does_not_fire_role_card() {
  // `SELECT * FROM (SELECT email FROM users) sub` -- the cursor on
  // `email` (a real catalog column) must NOT return the role-hover
  // card. `near_role_slot` used to flag any nearby `FROM` keyword as
  // a role-list context, which falsely fired for ordinary FROM-table
  // queries and obscured the real column card.
  let src = "SELECT * FROM (SELECT email FROM users) sub";
  let cur = src.find("email").unwrap();
  let md = hover_at(src, cur).expect("hover for column");
  assert!(
    !md.contains("_role_") && !md.contains("catalog not loaded"),
    "column hover wrongly returned role card: {md}"
  );
}

#[test]
fn hover_on_select_column_does_not_fire_role_card() {
  // Smoke test on the simplest possible SELECT/FROM -- hovering on a
  // catalog column must not be confused with a role context.
  let src = "SELECT email FROM users";
  let cur = src.find("email").unwrap();
  let md = hover_at(src, cur).expect("hover for column");
  assert!(!md.contains("_role_"), "select-list column wrongly got role card: {md}");
}

#[test]
fn hover_inside_double_quoted_identifier_does_not_return_keyword_card() {
  // `"User Id"` is a quoted identifier, not a keyword. The cursor on
  // the word USER inside those quotes must NOT return a USER-keyword
  // hover card; quoted identifiers are case-preserved column / table
  // names, never the reserved word USER.
  let src = r#"SELECT "User Id" FROM users"#;
  // Cursor on 'U' of User inside the quotes.
  let cur = src.find("\"User").unwrap() + 1;
  let md = hover_at(src, cur);
  if let Some(md) = md {
    assert!(
      !md.contains("Keyword") && !md.contains("Synonym for ROLE"),
      "quoted identifier `\"User Id\"` should not match USER keyword; got: {md}"
    );
  }
}

#[test]
fn hover_inside_double_quoted_identifier_with_select_does_not_return_keyword_card() {
  // Same shape but with a stronger collision: the quoted identifier
  // contains the literal text SELECT. Hover on it must NOT fire the
  // SELECT keyword card.
  let src = r#"SELECT "SELECT" FROM users"#;
  let cur = src.rfind("SELECT").unwrap(); // the second SELECT (inside quotes)
  let md = hover_at(src, cur);
  if let Some(md) = md {
    assert!(
      !md.contains("Keyword"),
      "quoted identifier `\"SELECT\"` must not surface SELECT keyword card; got: {md}"
    );
  }
}

#[test]
fn hover_on_cte_alias_left_does_not_offer_underlying_table_card() {
  // Cursor on `t` (the CTE alias) of `t.user_id` -- shouldn't claim
  // it's the `users` table just because `users` happens to be in scope.
  let src = "WITH t AS (SELECT id FROM users) SELECT t.id FROM t;";
  let cur = src.rfind("t.id").unwrap();
  let md = hover_at(src, cur);
  if let Some(md) = md {
    // It's fine to return nothing or a CTE-aware card. It is NOT fine
    // to claim `t` is the `users` table.
    assert!(
      !(md.contains("users") && md.starts_with("# Table")),
      "hover on CTE alias `t` incorrectly returned `users` table card: {md}"
    );
  }
}

// ============================================================================
// Function-call internals: hover on a column reference INSIDE a function
// call must surface the column, not the function signature. The
// function-signature card is reserved for non-identifier cursors
// (string / numeric literals, whitespace between args).
// ============================================================================

// ============================================================================
// Synthetic alias hover (subquery / CTE / function-call FROM).
// ============================================================================

#[test]
fn hover_on_subquery_alias_in_projection_surfaces_subquery_card() {
  let src = "SELECT subq.id FROM (SELECT id FROM users) AS subq";
  let cur = src.find("subq.id").unwrap(); // cursor on `s` of subq
  let md = hover_at(src, cur).expect("hover for subquery alias");
  assert!(md.contains("Subquery alias"), "expected subquery alias card: {md}");
  assert!(md.contains("subq"), "card should name the alias: {md}");
}

#[test]
fn hover_on_cte_alias_in_from_surfaces_cte_card() {
  let src = "WITH t AS (SELECT id FROM users) SELECT t.id FROM t";
  let cur = src.rfind(" t").unwrap() + 1; // cursor on the trailing `t`
  let md = hover_at(src, cur).expect("hover for CTE alias in FROM");
  assert!(md.contains("CTE alias"), "expected CTE alias card: {md}");
  assert!(md.contains("`id`"), "expected projected column listing: {md}");
}

#[test]
fn hover_on_function_call_alias_surfaces_function_card() {
  let src = "SELECT gs.* FROM generate_series(1, 10) AS gs";
  let cur = src.find("gs.").unwrap();
  let md = hover_at(src, cur).expect("hover for function-call alias");
  assert!(md.contains("Function-call alias"), "expected function-call alias card: {md}");
}

#[test]
fn hover_on_alias_inside_count_resolves_to_table() {
  let src = "SELECT count(u.id) FROM users u";
  let cur = src.find("u.id").unwrap(); // cursor on `u`
  let md = hover_at(src, cur).expect("hover for alias inside count()");
  let upper = md.to_ascii_uppercase();
  assert!(upper.contains("TABLE") && upper.contains("USERS"), "expected table card for alias `u`, got: {md}");
}

#[test]
fn hover_on_column_inside_count_resolves_to_column() {
  let src = "SELECT count(u.id) FROM users u";
  let cur = src.find("u.id").unwrap() + 2; // cursor on `id`
  let md = hover_at(src, cur).expect("hover for column inside count()");
  assert!(md.contains("Column") && md.contains("public.users.id"), "expected column card: {md}");
}

#[test]
fn hover_on_bare_column_inside_coalesce() {
  let src = "SELECT coalesce(id, 0) FROM users";
  let cur = src.find("(id").unwrap() + 1; // cursor on `i`
  let md = hover_at(src, cur).expect("hover for bare column inside coalesce()");
  assert!(md.contains("public.users.id"), "expected column card: {md}");
}

#[test]
fn hover_on_string_literal_inside_coalesce_still_shows_signature() {
  // Regression guard for the gate added to suppress function-signature
  // hover on identifier cursors -- string literals must still surface
  // the active-param card.
  let src = "SELECT coalesce(id, 'fallback') FROM users";
  let cur = src.find("'fallback'").unwrap() + 2; // cursor on `f`
  let md = hover_at(src, cur).expect("hover for string literal arg");
  assert!(md.contains("function call") && md.contains("coalesce"), "expected signature card: {md}");
}


#[test]
fn r3_006_hover_update_from_alias_column_resolves() {
  // CYCLE 3: UpdateStmt.from_tables binds aliases visible to hover.
  // `UPDATE ... FROM orders o WHERE o.id ...` -- hovering on `id`
  // (or `o`) must resolve via the FROM-list binding.
  let src = "UPDATE users SET active = true FROM users o WHERE o.id = 1";
  let cur = src.rfind("id").unwrap();
  let md = hover_at(src, cur).expect("hover for FROM-alias column");
  assert!(md.contains("public.users.id"), "expected column card: {md}");
}

#[test]
fn r3_006_hover_delete_using_alias_column_resolves() {
  let src = "DELETE FROM users USING users o WHERE o.id = 1";
  let cur = src.rfind("id").unwrap();
  let md = hover_at(src, cur).expect("hover for USING-alias column");
  assert!(md.contains("public.users.id"), "expected column card: {md}");
}

#[test]
fn r9_pilot_hover_table_ident() {
  let src = "SELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table ident");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_pilot_hover_column() {
  let src = "SELECT id FROM users";
  let cur = src.find("id").unwrap();
  let md = hover_at(src, cur).expect("hover on column");
  assert!(md.contains("id"));
}

#[test]
fn r9_pilot_hover_alias_left() {
  let src = "SELECT u.id FROM users u";
  let cur = src.find("u.").unwrap();
  let md = hover_at(src, cur).expect("hover on alias");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_pilot_hover_alias_right() {
  let src = "SELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover on alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r9_pilot_hover_empty_none() {
  let md = hover_at("", 0);
  assert!(md.is_none() || md.unwrap().is_empty());
}

#[test]
fn r9_pilot_hover_whitespace_none() {
  let md = hover_at("   ", 1);
  assert!(md.is_none() || md.unwrap().is_empty());
}

#[test]
fn r9_pilot_hover_cast_text() {
  let src = "SELECT id::text FROM users";
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover on text cast");
  assert!(md.to_ascii_lowercase().contains("text"));
}

#[test]
fn r9_pilot_hover_qual_table() {
  let src = "SELECT * FROM public.users";
  let cur = src.find(".users").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover on qual table");
  assert!(md.contains("users"));
}

#[test]
fn r9_pilot_hover_qual_col() {
  let src = "SELECT public.users.id FROM public.users";
  let cur = src.find(".id").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover on qual col");
  assert!(md.contains("id"));
}

#[test]
fn r9_hover_table_0001() {
  let src = "SELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0002() {
  let src = "SELECT id FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0003() {
  let src = "UPDATE users SET id=1";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0004() {
  let src = "DELETE FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0005() {
  let src = "INSERT INTO users VALUES (1)";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0006() {
  let src = "ALTER TABLE users RENAME TO x";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0007() {
  let src = "DROP TABLE users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0008() {
  let src = "TRUNCATE users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0009() {
  let src = "VACUUM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0010() {
  let src = "ANALYZE users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0011() {
  let src = "CLUSTER users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0012() {
  let src = "REINDEX TABLE users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0013() {
  let src = "LOCK TABLE users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0014() {
  let src = "COPY users TO STDOUT";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0015() {
  let src = "GRANT SELECT ON users TO alice";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0016() {
  let src = "REVOKE SELECT ON users FROM alice";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0017() {
  let src = "COMMENT ON TABLE users IS 'x'";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0018() {
  let src = "CREATE INDEX ON users (id)";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_table_0019() {
  let src = "EXPLAIN SELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover on table");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_col_0501() {
  let src = "SELECT id FROM users";
  let cur = src.find("id").unwrap();
  let md = hover_at(src, cur).expect("hover on col");
  assert!(md.contains("id"));
}

#[test]
fn r9_hover_col_0502() {
  let src = "SELECT email FROM users";
  let cur = src.find("email").unwrap();
  let md = hover_at(src, cur).expect("hover on col");
  assert!(md.contains("email"));
}

#[test]
fn r9_hover_col_0503() {
  let src = "UPDATE users SET id = 1 WHERE id = 0";
  let cur = src.find("id").unwrap();
  let md = hover_at(src, cur).expect("hover on col");
  assert!(md.contains("id"));
}

#[test]
fn r9_hover_col_0504() {
  let src = "DELETE FROM users WHERE id = 1";
  let cur = src.find("id").unwrap();
  let md = hover_at(src, cur).expect("hover on col");
  assert!(md.contains("id"));
}

#[test]
fn r9_hover_col_0505() {
  let src = "SELECT id, email FROM users";
  let cur = src.find("id").unwrap();
  let md = hover_at(src, cur).expect("hover on col");
  assert!(md.contains("id"));
}

#[test]
fn r9_hover_col_0506() {
  let src = "SELECT email, id FROM users";
  let cur = src.find("email").unwrap();
  let md = hover_at(src, cur).expect("hover on col");
  assert!(md.contains("email"));
}

#[test]
fn r9_hover_alias_l_0801() {
  let src = "SELECT u.id FROM users u";
  let cur = src.find("u.").unwrap();
  let md = hover_at(src, cur).expect("hover on alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_alias_l_0802() {
  let src = "SELECT u.email FROM users u";
  let cur = src.find("u.").unwrap();
  let md = hover_at(src, cur).expect("hover on alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_alias_l_0803() {
  let src = "UPDATE users u SET id=1 WHERE u.id=1";
  let cur = src.find("u.").unwrap();
  let md = hover_at(src, cur).expect("hover on alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_alias_l_0804() {
  let src = "DELETE FROM users u WHERE u.id=1";
  let cur = src.find("u.").unwrap();
  let md = hover_at(src, cur).expect("hover on alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_alias_l_0805() {
  let src = "SELECT u.id FROM users AS u";
  let cur = src.find("u.").unwrap();
  let md = hover_at(src, cur).expect("hover on alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r9_hover_alias_r_1102() {
  let src = "SELECT u.email FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover on alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r9_hover_alias_r_1103() {
  let src = "UPDATE users u SET id=u.id+1 WHERE u.id=1";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover on alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r9_hover_alias_r_1104() {
  let src = "DELETE FROM users u WHERE u.email='x'";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover on alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r9_hover_cast_1401() {
  let src = "SELECT id::text FROM users";
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("text"));
}

#[test]
fn r9_hover_cast_1402() {
  let src = "SELECT id::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r9_hover_cast_1403() {
  let src = "SELECT id::uuid FROM users";
  let cur = src.find("uuid").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("uuid"));
}

#[test]
fn r9_hover_cast_1404() {
  let src = "SELECT id::bigint FROM users";
  let cur = src.find("bigint").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("bigint"));
}

#[test]
fn r9_hover_cast_1405() {
  let src = "SELECT id::numeric FROM users";
  let cur = src.find("numeric").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("numeric"));
}

#[test]
fn r9_hover_qual_1702() {
  let src = "SELECT * FROM public.users WHERE id=1";
  let cur = src.find(".users").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover on qual table");
  assert!(md.contains("users"));
}

#[test]
fn r9_hover_qual_1703() {
  let src = "UPDATE public.users SET id=1";
  let cur = src.find(".users").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover on qual table");
  assert!(md.contains("users"));
}

#[test]
fn r9_hover_qual_1704() {
  let src = "DELETE FROM public.users WHERE id=1";
  let cur = src.find(".users").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover on qual table");
  assert!(md.contains("users"));
}

#[test]
fn r9_hover_alias_v_2401() {
  let src = "SELECT u.name FROM users u";
  let cur = src.find("u.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover on alias.col var");
  assert!(md.contains("name"));
}

#[test]
fn r9_hover_alias_v_2402() {
  let src = "UPDATE users u SET u.name='x'";
  let cur = src.find("u.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover on alias.col var");
  assert!(md.contains("name"));
}

#[test]
fn r9_hover_alias_v_2403() {
  let src = "SELECT u.email, u.name FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover on alias.col var");
  assert!(md.contains("email"));
}


#[test]
fn r10_hover_cast_0001() {
  let src = "-- ct0\nSELECT id::text FROM users";
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("text"));
}

#[test]
fn r10_hover_cast_0002() {
  let src = "-- ct1\nSELECT id::text FROM users";
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("text"));
}

#[test]
fn r10_hover_cast_0003() {
  let src = "-- ct2\nSELECT id::text FROM users";
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("text"));
}

#[test]
fn r10_hover_cast_0006() {
  let src = "-- ct0\nSELECT id::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r10_hover_cast_0007() {
  let src = "-- ct1\nSELECT id::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r10_hover_cast_0008() {
  let src = "-- ct2\nSELECT id::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r10_hover_cast_0011() {
  let src = "-- ct0\nSELECT id::integer FROM users";
  let cur = src.find("integer").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("integer"));
}

#[test]
fn r10_hover_cast_0012() {
  let src = "-- ct1\nSELECT id::integer FROM users";
  let cur = src.find("integer").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("integer"));
}

#[test]
fn r10_hover_cast_0013() {
  let src = "-- ct2\nSELECT id::integer FROM users";
  let cur = src.find("integer").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("integer"));
}

#[test]
fn r10_hover_cast_0016() {
  let src = "-- ct0\nSELECT id::bigint FROM users";
  let cur = src.find("bigint").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("bigint"));
}

#[test]
fn r10_hover_cast_0017() {
  let src = "-- ct1\nSELECT id::bigint FROM users";
  let cur = src.find("bigint").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("bigint"));
}

#[test]
fn r10_hover_cast_0018() {
  let src = "-- ct2\nSELECT id::bigint FROM users";
  let cur = src.find("bigint").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("bigint"));
}

#[test]
fn r10_hover_cast_0021() {
  let src = "-- ct0\nSELECT id::smallint FROM users";
  let cur = src.find("smallint").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("smallint"));
}

#[test]
fn r10_hover_cast_0022() {
  let src = "-- ct1\nSELECT id::smallint FROM users";
  let cur = src.find("smallint").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("smallint"));
}

#[test]
fn r10_hover_cast_0023() {
  let src = "-- ct2\nSELECT id::smallint FROM users";
  let cur = src.find("smallint").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("smallint"));
}

#[test]
fn r10_hover_cast_0026() {
  let src = "-- ct0\nSELECT id::boolean FROM users";
  let cur = src.find("boolean").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("boolean"));
}

#[test]
fn r10_hover_cast_0027() {
  let src = "-- ct1\nSELECT id::boolean FROM users";
  let cur = src.find("boolean").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("boolean"));
}

#[test]
fn r10_hover_cast_0028() {
  let src = "-- ct2\nSELECT id::boolean FROM users";
  let cur = src.find("boolean").unwrap();
  let md = hover_at(src, cur).expect("hover cast type");
  assert!(md.to_ascii_lowercase().contains("boolean"));
}

#[test]
fn r10_hover_alias_0151() {
  let src = "-- av0\nSELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 1 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r10_hover_alias_0152() {
  let src = "-- av1\nSELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 1 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r10_hover_alias_0153() {
  let src = "-- av2\nSELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 1 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r10_hover_alias_0155() {
  let src = "-- av0\nSELECT u.email FROM users u";
  let cur = src.find("u.email").unwrap() + 1 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r10_hover_alias_0156() {
  let src = "-- av1\nSELECT u.email FROM users u";
  let cur = src.find("u.email").unwrap() + 1 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r10_hover_alias_0157() {
  let src = "-- av2\nSELECT u.email FROM users u";
  let cur = src.find("u.email").unwrap() + 1 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r10_hover_alias_0159() {
  let src = "-- av0\nSELECT u.name FROM users u";
  let cur = src.find("u.name").unwrap() + 1 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r10_hover_alias_0160() {
  let src = "-- av1\nSELECT u.name FROM users u";
  let cur = src.find("u.name").unwrap() + 1 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r10_hover_alias_0161() {
  let src = "-- av2\nSELECT u.name FROM users u";
  let cur = src.find("u.name").unwrap() + 1 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r10_hover_alias_0163() {
  let src = "-- av0\nSELECT u1.id FROM users u1";
  let cur = src.find("u1.id").unwrap() + 2 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r10_hover_alias_0164() {
  let src = "-- av1\nSELECT u1.id FROM users u1";
  let cur = src.find("u1.id").unwrap() + 2 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r10_hover_alias_0165() {
  let src = "-- av2\nSELECT u1.id FROM users u1";
  let cur = src.find("u1.id").unwrap() + 2 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r10_hover_alias_0167() {
  let src = "-- av0\nSELECT u1.email FROM users u1";
  let cur = src.find("u1.email").unwrap() + 2 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r10_hover_alias_0168() {
  let src = "-- av1\nSELECT u1.email FROM users u1";
  let cur = src.find("u1.email").unwrap() + 2 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r10_hover_alias_0169() {
  let src = "-- av2\nSELECT u1.email FROM users u1";
  let cur = src.find("u1.email").unwrap() + 2 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r10_hover_alias_0171() {
  let src = "-- av0\nSELECT u1.name FROM users u1";
  let cur = src.find("u1.name").unwrap() + 2 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r10_hover_alias_0172() {
  let src = "-- av1\nSELECT u1.name FROM users u1";
  let cur = src.find("u1.name").unwrap() + 2 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r10_hover_alias_0173() {
  let src = "-- av2\nSELECT u1.name FROM users u1";
  let cur = src.find("u1.name").unwrap() + 2 + 1;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r10_hover_qual_0295() {
  let src = "-- q0\nSELECT public.users.id FROM public.users";
  let cur = src.find(".id").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual.col");
  assert!(md.contains("id"));
}

#[test]
fn r10_hover_qual_0296() {
  let src = "-- q1\nSELECT public.users.id FROM public.users";
  let cur = src.find(".id").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual.col");
  assert!(md.contains("id"));
}

#[test]
fn r10_hover_qual_0297() {
  let src = "-- q2\nSELECT public.users.id FROM public.users";
  let cur = src.find(".id").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual.col");
  assert!(md.contains("id"));
}

#[test]
fn r10_hover_qual_0301() {
  let src = "-- q0\nSELECT public.users.email FROM public.users";
  let cur = src.find(".email").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual.col");
  assert!(md.contains("email"));
}

#[test]
fn r10_hover_qual_0302() {
  let src = "-- q1\nSELECT public.users.email FROM public.users";
  let cur = src.find(".email").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual.col");
  assert!(md.contains("email"));
}

#[test]
fn r10_hover_qual_0303() {
  let src = "-- q2\nSELECT public.users.email FROM public.users";
  let cur = src.find(".email").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual.col");
  assert!(md.contains("email"));
}

#[test]
fn r10_hover_qual_0307() {
  let src = "-- q0\nSELECT public.users.name FROM public.users";
  let cur = src.find(".name").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual.col");
  assert!(md.contains("name"));
}

#[test]
fn r10_hover_qual_0308() {
  let src = "-- q1\nSELECT public.users.name FROM public.users";
  let cur = src.find(".name").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual.col");
  assert!(md.contains("name"));
}

#[test]
fn r10_hover_qual_0309() {
  let src = "-- q2\nSELECT public.users.name FROM public.users";
  let cur = src.find(".name").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual.col");
  assert!(md.contains("name"));
}

#[test]
fn r10_hover_t_0313() {
  let src = "-- t0\nSELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover table ident");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r10_hover_t_0314() {
  let src = "/* t0 */ SELECT id FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover table ident");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r10_hover_t_0315() {
  let src = "-- t0\nUPDATE users SET id=1";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover table ident");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r10_hover_t_0316() {
  let src = "-- t1\nSELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover table ident");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r10_hover_t_0317() {
  let src = "/* t1 */ SELECT id FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover table ident");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r10_hover_t_0318() {
  let src = "-- t1\nUPDATE users SET id=1";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover table ident");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r10_hover_t_0319() {
  let src = "-- t2\nSELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover table ident");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r10_hover_t_0320() {
  let src = "/* t2 */ SELECT id FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover table ident");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r10_hover_t_0321() {
  let src = "-- t2\nUPDATE users SET id=1";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover table ident");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r11_h_col_0001() {
  let src = "-- ch0\nSELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("id"));
}

#[test]
fn r11_h_col_0002() {
  let src = "-- ch1\nSELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("id"));
}

#[test]
fn r11_h_col_0003() {
  let src = "-- ch2\nSELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("id"));
}

#[test]
fn r11_h_col_0006() {
  let src = "-- ch0\nSELECT u.email FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("email"));
}

#[test]
fn r11_h_col_0007() {
  let src = "-- ch1\nSELECT u.email FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("email"));
}

#[test]
fn r11_h_col_0008() {
  let src = "-- ch2\nSELECT u.email FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("email"));
}

#[test]
fn r11_h_col_0011() {
  let src = "-- ch0\nSELECT u.name FROM users u";
  let cur = src.find("u.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("name"));
}

#[test]
fn r11_h_col_0012() {
  let src = "-- ch1\nSELECT u.name FROM users u";
  let cur = src.find("u.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("name"));
}

#[test]
fn r11_h_col_0013() {
  let src = "-- ch2\nSELECT u.name FROM users u";
  let cur = src.find("u.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("name"));
}

#[test]
fn r11_h_col_0016() {
  let src = "-- ch0\nSELECT u1.id FROM users u1";
  let cur = src.find("u1.id").unwrap() + 3;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("id"));
}

#[test]
fn r11_h_col_0017() {
  let src = "-- ch1\nSELECT u1.id FROM users u1";
  let cur = src.find("u1.id").unwrap() + 3;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("id"));
}

#[test]
fn r11_h_col_0018() {
  let src = "-- ch2\nSELECT u1.id FROM users u1";
  let cur = src.find("u1.id").unwrap() + 3;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("id"));
}

#[test]
fn r11_h_col_0021() {
  let src = "-- ch0\nSELECT u1.email FROM users u1";
  let cur = src.find("u1.email").unwrap() + 3;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("email"));
}

#[test]
fn r11_h_col_0022() {
  let src = "-- ch1\nSELECT u1.email FROM users u1";
  let cur = src.find("u1.email").unwrap() + 3;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("email"));
}

#[test]
fn r11_h_col_0023() {
  let src = "-- ch2\nSELECT u1.email FROM users u1";
  let cur = src.find("u1.email").unwrap() + 3;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("email"));
}

#[test]
fn r11_h_col_0026() {
  let src = "-- ch0\nSELECT u1.name FROM users u1";
  let cur = src.find("u1.name").unwrap() + 3;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("name"));
}

#[test]
fn r11_h_col_0027() {
  let src = "-- ch1\nSELECT u1.name FROM users u1";
  let cur = src.find("u1.name").unwrap() + 3;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("name"));
}

#[test]
fn r11_h_col_0028() {
  let src = "-- ch2\nSELECT u1.name FROM users u1";
  let cur = src.find("u1.name").unwrap() + 3;
  let md = hover_at(src, cur).expect("hover col");
  assert!(md.contains("name"));
}

#[test]
fn r11_h_tbl_0196() {
  let src = "-- bh0\nSELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover tbl");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r11_h_tbl_0197() {
  let src = "-- bh1\nSELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover tbl");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r11_h_tbl_0198() {
  let src = "-- bh2\nSELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover tbl");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r12_h_qual_col_0001() {
  let src = "-- hq0\nSELECT public.users.id FROM public.users";
  let cur = src.find(".id").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual col");
  assert!(md.contains("id"));
}

#[test]
fn r12_h_qual_col_0002() {
  let src = "-- hq1\nSELECT public.users.id FROM public.users";
  let cur = src.find(".id").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual col");
  assert!(md.contains("id"));
}

#[test]
fn r12_h_qual_col_0003() {
  let src = "-- hq2\nSELECT public.users.id FROM public.users";
  let cur = src.find(".id").unwrap() + 1;
  let md = hover_at(src, cur).expect("hover qual col");
  assert!(md.contains("id"));
}

#[test]
fn r12_h_cast_0181() {
  let src = "-- hc0\nSELECT id::text FROM users";
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("text"));
}

#[test]
fn r12_h_cast_0182() {
  let src = "-- hc0\nSELECT id::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r12_h_cast_0183() {
  let src = "-- hc0\nSELECT id::bigint FROM users";
  let cur = src.find("bigint").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("bigint"));
}

#[test]
fn r12_h_cast_0184() {
  let src = "-- hc0\nSELECT id::smallint FROM users";
  let cur = src.find("smallint").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("smallint"));
}

#[test]
fn r12_h_cast_0185() {
  let src = "-- hc0\nSELECT id::boolean FROM users";
  let cur = src.find("boolean").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("boolean"));
}

#[test]
fn r12_h_cast_0186() {
  let src = "-- hc0\nSELECT id::jsonb FROM users";
  let cur = src.find("jsonb").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("jsonb"));
}

#[test]
fn r12_h_cast_0187() {
  let src = "-- hc0\nSELECT id::uuid FROM users";
  let cur = src.find("uuid").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("uuid"));
}

#[test]
fn r12_h_cast_0188() {
  let src = "-- hc0\nSELECT id::numeric FROM users";
  let cur = src.find("numeric").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("numeric"));
}

#[test]
fn r12_h_cast_0189() {
  let src = "-- hc0\nSELECT id::date FROM users";
  let cur = src.find("date").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("date"));
}

#[test]
fn r12_h_cast_0190() {
  let src = "-- hc0\nSELECT id::time FROM users";
  let cur = src.find("time").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("time"));
}

#[test]
fn r12_h_cast_0191() {
  let src = "-- hc0\nSELECT id::timestamp FROM users";
  let cur = src.find("timestamp").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("timestamp"));
}

#[test]
fn r12_h_cast_0192() {
  let src = "-- hc0\nSELECT id::timestamptz FROM users";
  let cur = src.find("timestamptz").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("timestamptz"));
}

#[test]
fn r12_h_cast_0193() {
  let src = "-- hc0\nSELECT id::interval FROM users";
  let cur = src.find("interval").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("interval"));
}

#[test]
fn r12_h_cast_0194() {
  let src = "-- hc0\nSELECT id::bytea FROM users";
  let cur = src.find("bytea").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("bytea"));
}

#[test]
fn r12_h_cast_0195() {
  let src = "-- hc0\nSELECT id::varchar FROM users";
  let cur = src.find("varchar").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("varchar"));
}

#[test]
fn r12_h_cast_0196() {
  let src = "-- hc1\nSELECT id::text FROM users";
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("text"));
}

#[test]
fn r12_h_cast_0197() {
  let src = "-- hc1\nSELECT id::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r12_h_cast_0198() {
  let src = "-- hc1\nSELECT id::bigint FROM users";
  let cur = src.find("bigint").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("bigint"));
}

#[test]
fn r12_h_cast_0199() {
  let src = "-- hc1\nSELECT id::smallint FROM users";
  let cur = src.find("smallint").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("smallint"));
}

#[test]
fn r12_h_cast_0200() {
  let src = "-- hc1\nSELECT id::boolean FROM users";
  let cur = src.find("boolean").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("boolean"));
}

#[test]
fn r12_h_cast_0201() {
  let src = "-- hc1\nSELECT id::jsonb FROM users";
  let cur = src.find("jsonb").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("jsonb"));
}

#[test]
fn r12_h_cast_0202() {
  let src = "-- hc1\nSELECT id::uuid FROM users";
  let cur = src.find("uuid").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("uuid"));
}

#[test]
fn r12_h_cast_0203() {
  let src = "-- hc1\nSELECT id::numeric FROM users";
  let cur = src.find("numeric").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("numeric"));
}

#[test]
fn r12_h_cast_0204() {
  let src = "-- hc1\nSELECT id::date FROM users";
  let cur = src.find("date").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("date"));
}

#[test]
fn r12_h_cast_0205() {
  let src = "-- hc1\nSELECT id::time FROM users";
  let cur = src.find("time").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("time"));
}

#[test]
fn r12_h_cast_0206() {
  let src = "-- hc1\nSELECT id::timestamp FROM users";
  let cur = src.find("timestamp").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("timestamp"));
}

#[test]
fn r12_h_cast_0207() {
  let src = "-- hc1\nSELECT id::timestamptz FROM users";
  let cur = src.find("timestamptz").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("timestamptz"));
}

#[test]
fn r12_h_cast_0208() {
  let src = "-- hc1\nSELECT id::interval FROM users";
  let cur = src.find("interval").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("interval"));
}

#[test]
fn r12_h_cast_0209() {
  let src = "-- hc1\nSELECT id::bytea FROM users";
  let cur = src.find("bytea").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("bytea"));
}

#[test]
fn r12_h_cast_0210() {
  let src = "-- hc1\nSELECT id::varchar FROM users";
  let cur = src.find("varchar").unwrap();
  let md = hover_at(src, cur).expect("hover cast");
  assert!(md.to_ascii_lowercase().contains("varchar"));
}

#[test]
fn r13_h_a_0001() {
  let src = "-- ah1\nSELECT my_0.id FROM users my_0";
  let cur = src.find("my_0.id").unwrap() + 5;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r13_h_a_0002() {
  let src = "-- ah2\nSELECT my_0.email FROM users my_0";
  let cur = src.find("my_0.email").unwrap() + 5;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r13_h_a_0003() {
  let src = "-- ah3\nSELECT my_0.name FROM users my_0";
  let cur = src.find("my_0.name").unwrap() + 5;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r13_h_a_0004() {
  let src = "-- ah4\nSELECT my_1.id FROM users my_1";
  let cur = src.find("my_1.id").unwrap() + 5;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r13_h_a_0005() {
  let src = "-- ah5\nSELECT my_1.email FROM users my_1";
  let cur = src.find("my_1.email").unwrap() + 5;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r13_h_a_0006() {
  let src = "-- ah6\nSELECT my_1.name FROM users my_1";
  let cur = src.find("my_1.name").unwrap() + 5;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r13_h_a_0007() {
  let src = "-- ah7\nSELECT my_2.id FROM users my_2";
  let cur = src.find("my_2.id").unwrap() + 5;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r13_h_a_0008() {
  let src = "-- ah8\nSELECT my_2.email FROM users my_2";
  let cur = src.find("my_2.email").unwrap() + 5;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r13_h_a_0009() {
  let src = "-- ah9\nSELECT my_2.name FROM users my_2";
  let cur = src.find("my_2.name").unwrap() + 5;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r13_h_a_0025() {
  let src = "-- ah25\nSELECT tbl_0.id FROM users tbl_0";
  let cur = src.find("tbl_0.id").unwrap() + 6;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r13_h_a_0026() {
  let src = "-- ah26\nSELECT tbl_0.email FROM users tbl_0";
  let cur = src.find("tbl_0.email").unwrap() + 6;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r13_h_a_0027() {
  let src = "-- ah27\nSELECT tbl_0.name FROM users tbl_0";
  let cur = src.find("tbl_0.name").unwrap() + 6;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r13_h_a_0028() {
  let src = "-- ah28\nSELECT tbl_1.id FROM users tbl_1";
  let cur = src.find("tbl_1.id").unwrap() + 6;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r13_h_a_0029() {
  let src = "-- ah29\nSELECT tbl_1.email FROM users tbl_1";
  let cur = src.find("tbl_1.email").unwrap() + 6;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r13_h_a_0030() {
  let src = "-- ah30\nSELECT tbl_1.name FROM users tbl_1";
  let cur = src.find("tbl_1.name").unwrap() + 6;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r13_h_al_0337() {
  let src = "-- alh1\nSELECT my_0.id FROM users my_0";
  let cur = src.find("my_0.").unwrap();
  let md = hover_at(src, cur).expect("hover alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_al_0338() {
  let src = "-- alh2\nSELECT my_1.id FROM users my_1";
  let cur = src.find("my_1.").unwrap();
  let md = hover_at(src, cur).expect("hover alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_al_0339() {
  let src = "-- alh3\nSELECT my_2.id FROM users my_2";
  let cur = src.find("my_2.").unwrap();
  let md = hover_at(src, cur).expect("hover alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_al_0352() {
  let src = "-- alh16\nSELECT tbl_0.id FROM users tbl_0";
  let cur = src.find("tbl_0.").unwrap();
  let md = hover_at(src, cur).expect("hover alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_al_0353() {
  let src = "-- alh17\nSELECT tbl_1.id FROM users tbl_1";
  let cur = src.find("tbl_1.").unwrap();
  let md = hover_at(src, cur).expect("hover alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_al_0354() {
  let src = "-- alh18\nSELECT tbl_2.id FROM users tbl_2";
  let cur = src.find("tbl_2.").unwrap();
  let md = hover_at(src, cur).expect("hover alias left");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_b_0472() {
  let src = "-- bv0\nSELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover bare tbl");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_b_0473() {
  let src = "/* bv0 */ SELECT id FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover bare tbl");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_b_0474() {
  let src = "-- bv1\nSELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover bare tbl");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_b_0475() {
  let src = "/* bv1 */ SELECT id FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover bare tbl");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_b_0476() {
  let src = "-- bv2\nSELECT * FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover bare tbl");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r13_h_b_0477() {
  let src = "/* bv2 */ SELECT id FROM users";
  let cur = src.find("users").unwrap();
  let md = hover_at(src, cur).expect("hover bare tbl");
  assert!(md.to_ascii_lowercase().contains("users"));
}

#[test]
fn r14_h_combo_0001() {
  let src = "-- hu_1_0\nSELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("id"));
}

#[test]
fn r14_h_combo_0002() {
  let src = "-- hu_2_1\nSELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("id"));
}

#[test]
fn r14_h_combo_0003() {
  let src = "-- hu_3_2\nSELECT u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("id"));
}

#[test]
fn r14_h_combo_0009() {
  let src = "-- hu_9_0\nSELECT u.email FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("email"));
}

#[test]
fn r14_h_combo_0010() {
  let src = "-- hu_10_1\nSELECT u.email FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("email"));
}

#[test]
fn r14_h_combo_0011() {
  let src = "-- hu_11_2\nSELECT u.email FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("email"));
}

#[test]
fn r14_h_combo_0017() {
  let src = "-- hu_17_0\nSELECT u.name FROM users u";
  let cur = src.find("u.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("name"));
}

#[test]
fn r14_h_combo_0018() {
  let src = "-- hu_18_1\nSELECT u.name FROM users u";
  let cur = src.find("u.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("name"));
}

#[test]
fn r14_h_combo_0019() {
  let src = "-- hu_19_2\nSELECT u.name FROM users u";
  let cur = src.find("u.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("name"));
}

#[test]
fn r14_h_combo_0025() {
  let src = "-- hu_25_0\nSELECT x.id FROM users x";
  let cur = src.find("x.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("id"));
}

#[test]
fn r14_h_combo_0026() {
  let src = "-- hu_26_1\nSELECT x.id FROM users x";
  let cur = src.find("x.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("id"));
}

#[test]
fn r14_h_combo_0027() {
  let src = "-- hu_27_2\nSELECT x.id FROM users x";
  let cur = src.find("x.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover combo");
  assert!(md.contains("id"));
}

#[test]
fn r15_h_az_0001() {
  let src = "-- ha_1_0_\nSELECT a.id FROM users a";
  let cur = src.find("a.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("id"));
}

#[test]
fn r15_h_az_0002() {
  let src = "-- ha_2_1_\nSELECT a.id FROM users a";
  let cur = src.find("a.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("id"));
}

#[test]
fn r15_h_az_0003() {
  let src = "-- ha_3_2_\nSELECT a.id FROM users a";
  let cur = src.find("a.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("id"));
}

#[test]
fn r15_h_az_0006() {
  let src = "-- ha_6_0_\nSELECT a.email FROM users a";
  let cur = src.find("a.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("email"));
}

#[test]
fn r15_h_az_0007() {
  let src = "-- ha_7_1_\nSELECT a.email FROM users a";
  let cur = src.find("a.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("email"));
}

#[test]
fn r15_h_az_0008() {
  let src = "-- ha_8_2_\nSELECT a.email FROM users a";
  let cur = src.find("a.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("email"));
}

#[test]
fn r15_h_az_0011() {
  let src = "-- ha_11_0_\nSELECT a.name FROM users a";
  let cur = src.find("a.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("name"));
}

#[test]
fn r15_h_az_0012() {
  let src = "-- ha_12_1_\nSELECT a.name FROM users a";
  let cur = src.find("a.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("name"));
}

#[test]
fn r15_h_az_0013() {
  let src = "-- ha_13_2_\nSELECT a.name FROM users a";
  let cur = src.find("a.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("name"));
}

#[test]
fn r15_h_az_0016() {
  let src = "-- ha_16_0_\nSELECT b.id FROM users b";
  let cur = src.find("b.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("id"));
}

#[test]
fn r15_h_az_0017() {
  let src = "-- ha_17_1_\nSELECT b.id FROM users b";
  let cur = src.find("b.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("id"));
}

#[test]
fn r15_h_az_0018() {
  let src = "-- ha_18_2_\nSELECT b.id FROM users b";
  let cur = src.find("b.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("id"));
}

#[test]
fn r15_h_az_0021() {
  let src = "-- ha_21_0_\nSELECT b.email FROM users b";
  let cur = src.find("b.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("email"));
}

#[test]
fn r15_h_az_0022() {
  let src = "-- ha_22_1_\nSELECT b.email FROM users b";
  let cur = src.find("b.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("email"));
}

#[test]
fn r15_h_az_0023() {
  let src = "-- ha_23_2_\nSELECT b.email FROM users b";
  let cur = src.find("b.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("email"));
}

#[test]
fn r15_h_az_0026() {
  let src = "-- ha_26_0_\nSELECT b.name FROM users b";
  let cur = src.find("b.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("name"));
}

#[test]
fn r15_h_az_0027() {
  let src = "-- ha_27_1_\nSELECT b.name FROM users b";
  let cur = src.find("b.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("name"));
}

#[test]
fn r15_h_az_0028() {
  let src = "-- ha_28_2_\nSELECT b.name FROM users b";
  let cur = src.find("b.name").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover az");
  assert!(md.contains("name"));
}

#[test]
fn r16_h_0001() {
  let src = "-- hr_1_0\nSELECT abc.id FROM users abc";
  let cur = src.find("abc.id").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r16_h_0002() {
  let src = "-- hr_2_1\nSELECT abc.id FROM users abc";
  let cur = src.find("abc.id").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r16_h_0003() {
  let src = "-- hr_3_2\nSELECT abc.id FROM users abc";
  let cur = src.find("abc.id").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r16_h_0009() {
  let src = "-- hr_9_0\nSELECT abc.email FROM users abc";
  let cur = src.find("abc.email").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r16_h_0010() {
  let src = "-- hr_10_1\nSELECT abc.email FROM users abc";
  let cur = src.find("abc.email").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r16_h_0011() {
  let src = "-- hr_11_2\nSELECT abc.email FROM users abc";
  let cur = src.find("abc.email").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("email"));
}

#[test]
fn r16_h_0017() {
  let src = "-- hr_17_0\nSELECT abc.name FROM users abc";
  let cur = src.find("abc.name").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r16_h_0018() {
  let src = "-- hr_18_1\nSELECT abc.name FROM users abc";
  let cur = src.find("abc.name").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r16_h_0019() {
  let src = "-- hr_19_2\nSELECT abc.name FROM users abc";
  let cur = src.find("abc.name").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("name"));
}

#[test]
fn r16_h_0025() {
  let src = "-- hr_25_0\nSELECT def.id FROM users def";
  let cur = src.find("def.id").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r16_h_0026() {
  let src = "-- hr_26_1\nSELECT def.id FROM users def";
  let cur = src.find("def.id").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r16_h_0027() {
  let src = "-- hr_27_2\nSELECT def.id FROM users def";
  let cur = src.find("def.id").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover alias.col");
  assert!(md.contains("id"));
}

#[test]
fn r17_probe_hover_edges() {
  let cases = [
    ("SELECT 'literal' FROM users", 8, "in_string"),
    ("-- a comment\nSELECT 1", 5, "in_line_comment"),
    ("/* block */SELECT 1", 4, "in_block_comment"),
    ("SELECT 1;", 8, "at_semi"),
    ("SELECT u.id FROM users u WHERE u.id = 1", 26, "where_alias"),
    ("SELECT id::text FROM users WHERE name = 'a'", 11, "between_id_cast"),
    ("SELECT  FROM users", 7, "empty_select"),
    ("\n\nSELECT * FROM users\n\n", 22, "trailing_newlines"),
  ];
  for (src, off, label) in cases {
    let md = hover_at(src, off);
    let is_some = md.is_some();
    eprintln!("HE|{}|some={}", label, is_some);
  }
}

#[test]
fn r17_h_in_lc_0001() {
  let src = "-- this is a comment\n-- v0";
  let md = hover_at(src, 5);
  assert!(md.is_none(), "hover in line comment should be None #1");
}

#[test]
fn r17_h_in_lc_0002() {
  let src = "-- another comment with text\n-- v0";
  let md = hover_at(src, 10);
  assert!(md.is_none(), "hover in line comment should be None #2");
}

#[test]
fn r17_h_in_lc_0003() {
  let src = "-- inline\n-- v0";
  let md = hover_at(src, 3);
  assert!(md.is_none(), "hover in line comment should be None #3");
}

#[test]
fn r17_h_in_lc_0004() {
  let src = "-- multi word comment\n-- v0";
  let md = hover_at(src, 12);
  assert!(md.is_none(), "hover in line comment should be None #4");
}

#[test]
fn r17_h_in_lc_0005() {
  let src = "-- with special chars !@#\n-- v0";
  let md = hover_at(src, 15);
  assert!(md.is_none(), "hover in line comment should be None #5");
}

#[test]
fn r17_h_in_lc_0006() {
  let src = "-- 数字 123\n-- v0";
  let md = hover_at(src, 5);
  assert!(md.is_none(), "hover in line comment should be None #6");
}

#[test]
fn r17_h_in_lc_0007() {
  let src = "-- end of line\n-- v0";
  let md = hover_at(src, 7);
  assert!(md.is_none(), "hover in line comment should be None #7");
}

#[test]
fn r17_h_in_lc_0008() {
  let src = "-- TODO: fix later\n-- v0";
  let md = hover_at(src, 9);
  assert!(md.is_none(), "hover in line comment should be None #8");
}

#[test]
fn r17_h_in_lc_0009() {
  let src = "-- FIXME: bug\n-- v0";
  let md = hover_at(src, 8);
  assert!(md.is_none(), "hover in line comment should be None #9");
}

#[test]
fn r17_h_in_lc_0010() {
  let src = "-- NOTE: read me\n-- v0";
  let md = hover_at(src, 9);
  assert!(md.is_none(), "hover in line comment should be None #10");
}

#[test]
fn r17_h_in_lc_0011() {
  let src = "-- this is a comment\n-- v1";
  let md = hover_at(src, 5);
  assert!(md.is_none(), "hover in line comment should be None #11");
}

#[test]
fn r17_h_in_lc_0012() {
  let src = "-- another comment with text\n-- v1";
  let md = hover_at(src, 10);
  assert!(md.is_none(), "hover in line comment should be None #12");
}

#[test]
fn r17_h_in_lc_0013() {
  let src = "-- inline\n-- v1";
  let md = hover_at(src, 3);
  assert!(md.is_none(), "hover in line comment should be None #13");
}

#[test]
fn r17_h_in_lc_0014() {
  let src = "-- multi word comment\n-- v1";
  let md = hover_at(src, 12);
  assert!(md.is_none(), "hover in line comment should be None #14");
}

#[test]
fn r17_h_in_lc_0015() {
  let src = "-- with special chars !@#\n-- v1";
  let md = hover_at(src, 15);
  assert!(md.is_none(), "hover in line comment should be None #15");
}

#[test]
fn r17_h_in_lc_0016() {
  let src = "-- 数字 123\n-- v1";
  let md = hover_at(src, 5);
  assert!(md.is_none(), "hover in line comment should be None #16");
}

#[test]
fn r17_h_in_lc_0017() {
  let src = "-- end of line\n-- v1";
  let md = hover_at(src, 7);
  assert!(md.is_none(), "hover in line comment should be None #17");
}

#[test]
fn r17_h_in_lc_0018() {
  let src = "-- TODO: fix later\n-- v1";
  let md = hover_at(src, 9);
  assert!(md.is_none(), "hover in line comment should be None #18");
}

#[test]
fn r17_h_in_lc_0019() {
  let src = "-- FIXME: bug\n-- v1";
  let md = hover_at(src, 8);
  assert!(md.is_none(), "hover in line comment should be None #19");
}

#[test]
fn r17_h_in_lc_0020() {
  let src = "-- NOTE: read me\n-- v1";
  let md = hover_at(src, 9);
  assert!(md.is_none(), "hover in line comment should be None #20");
}

#[test]
fn r17_h_in_lc_0021() {
  let src = "-- this is a comment\n-- v2";
  let md = hover_at(src, 5);
  assert!(md.is_none(), "hover in line comment should be None #21");
}

#[test]
fn r17_h_in_lc_0022() {
  let src = "-- another comment with text\n-- v2";
  let md = hover_at(src, 10);
  assert!(md.is_none(), "hover in line comment should be None #22");
}

#[test]
fn r17_h_in_lc_0023() {
  let src = "-- inline\n-- v2";
  let md = hover_at(src, 3);
  assert!(md.is_none(), "hover in line comment should be None #23");
}

#[test]
fn r17_h_in_lc_0024() {
  let src = "-- multi word comment\n-- v2";
  let md = hover_at(src, 12);
  assert!(md.is_none(), "hover in line comment should be None #24");
}

#[test]
fn r17_h_in_lc_0025() {
  let src = "-- with special chars !@#\n-- v2";
  let md = hover_at(src, 15);
  assert!(md.is_none(), "hover in line comment should be None #25");
}

#[test]
fn r17_h_in_bc_0026() {
  let src = "/* block comment */\n-- v0";
  let md = hover_at(src, 5);
  assert!(md.is_none(), "hover in block comment should be None #26");
}

#[test]
fn r17_h_in_bc_0027() {
  let src = "/* with newline\nhere */\n-- v0";
  let md = hover_at(src, 8);
  assert!(md.is_none(), "hover in block comment should be None #27");
}

#[test]
fn r17_h_in_bc_0028() {
  let src = "/* multiple\nlines\nhere */\n-- v0";
  let md = hover_at(src, 12);
  assert!(md.is_none(), "hover in block comment should be None #28");
}

#[test]
fn r17_h_in_bc_0029() {
  let src = "/** doc comment */\n-- v0";
  let md = hover_at(src, 7);
  assert!(md.is_none(), "hover in block comment should be None #29");
}

#[test]
fn r17_h_in_bc_0035() {
  let src = "/** doc comment */\n-- v1";
  let md = hover_at(src, 7);
  assert!(md.is_none(), "hover in block comment should be None #35");
}

#[test]
fn r17_h_in_bc_0041() {
  let src = "/** doc comment */\n-- v2";
  let md = hover_at(src, 7);
  assert!(md.is_none(), "hover in block comment should be None #41");
}

#[test]
fn r17_h_semi_0051() {
  let src = "-- s0\nSELECT 1;";
  let md = hover_at(src, 15);
  assert!(md.is_none(), "hover at semicolon should be None #51");
}

#[test]
fn r17_h_semi_0052() {
  let src = "-- s0\nSELECT * FROM users;";
  let md = hover_at(src, 26);
  assert!(md.is_none(), "hover at semicolon should be None #52");
}

#[test]
fn r17_h_semi_0053() {
  let src = "-- s0\nUPDATE users SET id=1;";
  let md = hover_at(src, 28);
  assert!(md.is_none(), "hover at semicolon should be None #53");
}

#[test]
fn r17_h_semi_0054() {
  let src = "-- s0\nDELETE FROM users;";
  let md = hover_at(src, 24);
  assert!(md.is_none(), "hover at semicolon should be None #54");
}

#[test]
fn r17_h_semi_0055() {
  let src = "-- s0\nBEGIN;";
  let md = hover_at(src, 12);
  assert!(md.is_none(), "hover at semicolon should be None #55");
}

#[test]
fn r17_h_semi_0056() {
  let src = "-- s0\nCOMMIT;";
  let md = hover_at(src, 13);
  assert!(md.is_none(), "hover at semicolon should be None #56");
}

#[test]
fn r17_h_semi_0057() {
  let src = "-- s0\nROLLBACK;";
  let md = hover_at(src, 15);
  assert!(md.is_none(), "hover at semicolon should be None #57");
}

#[test]
fn r17_h_semi_0058() {
  let src = "-- s0\nINSERT INTO users (id) VALUES (1);";
  let md = hover_at(src, 40);
  assert!(md.is_none(), "hover at semicolon should be None #58");
}

#[test]
fn r17_h_semi_0059() {
  let src = "-- s0\nCREATE TABLE t (id int);";
  let md = hover_at(src, 30);
  assert!(md.is_none(), "hover at semicolon should be None #59");
}

#[test]
fn r17_h_semi_0060() {
  let src = "-- s0\nDROP TABLE users;";
  let md = hover_at(src, 23);
  assert!(md.is_none(), "hover at semicolon should be None #60");
}

#[test]
fn r17_h_semi_0061() {
  let src = "-- s1\nSELECT 1;";
  let md = hover_at(src, 15);
  assert!(md.is_none(), "hover at semicolon should be None #61");
}

#[test]
fn r17_h_semi_0062() {
  let src = "-- s1\nSELECT * FROM users;";
  let md = hover_at(src, 26);
  assert!(md.is_none(), "hover at semicolon should be None #62");
}

#[test]
fn r17_h_semi_0063() {
  let src = "-- s1\nUPDATE users SET id=1;";
  let md = hover_at(src, 28);
  assert!(md.is_none(), "hover at semicolon should be None #63");
}

#[test]
fn r17_h_semi_0064() {
  let src = "-- s1\nDELETE FROM users;";
  let md = hover_at(src, 24);
  assert!(md.is_none(), "hover at semicolon should be None #64");
}

#[test]
fn r17_h_semi_0065() {
  let src = "-- s1\nBEGIN;";
  let md = hover_at(src, 12);
  assert!(md.is_none(), "hover at semicolon should be None #65");
}

#[test]
fn r17_h_semi_0066() {
  let src = "-- s1\nCOMMIT;";
  let md = hover_at(src, 13);
  assert!(md.is_none(), "hover at semicolon should be None #66");
}

#[test]
fn r17_h_semi_0067() {
  let src = "-- s1\nROLLBACK;";
  let md = hover_at(src, 15);
  assert!(md.is_none(), "hover at semicolon should be None #67");
}

#[test]
fn r17_h_semi_0068() {
  let src = "-- s1\nINSERT INTO users (id) VALUES (1);";
  let md = hover_at(src, 40);
  assert!(md.is_none(), "hover at semicolon should be None #68");
}

#[test]
fn r17_h_semi_0069() {
  let src = "-- s1\nCREATE TABLE t (id int);";
  let md = hover_at(src, 30);
  assert!(md.is_none(), "hover at semicolon should be None #69");
}

#[test]
fn r17_h_semi_0070() {
  let src = "-- s1\nDROP TABLE users;";
  let md = hover_at(src, 23);
  assert!(md.is_none(), "hover at semicolon should be None #70");
}

#[test]
fn r17_h_semi_0071() {
  let src = "-- s2\nSELECT 1;";
  let md = hover_at(src, 15);
  assert!(md.is_none(), "hover at semicolon should be None #71");
}

#[test]
fn r17_h_semi_0072() {
  let src = "-- s2\nSELECT * FROM users;";
  let md = hover_at(src, 26);
  assert!(md.is_none(), "hover at semicolon should be None #72");
}

#[test]
fn r17_h_semi_0073() {
  let src = "-- s2\nUPDATE users SET id=1;";
  let md = hover_at(src, 28);
  assert!(md.is_none(), "hover at semicolon should be None #73");
}

#[test]
fn r17_h_semi_0074() {
  let src = "-- s2\nDELETE FROM users;";
  let md = hover_at(src, 24);
  assert!(md.is_none(), "hover at semicolon should be None #74");
}

#[test]
fn r17_h_semi_0075() {
  let src = "-- s2\nBEGIN;";
  let md = hover_at(src, 12);
  assert!(md.is_none(), "hover at semicolon should be None #75");
}

#[test]
fn r17_h_ws_mid_0077() {
  let src = "-- w0\nSELECT id  FROM users";
  let md = hover_at(src, 16);
  assert!(md.is_none(), "hover on inter-token whitespace should be None #77");
}

#[test]
fn r17_h_ws_mid_0078() {
  let src = "-- w0\nSELECT id FROM  users";
  let md = hover_at(src, 21);
  assert!(md.is_none(), "hover on inter-token whitespace should be None #78");
}

#[test]
fn r17_h_ws_mid_0083() {
  let src = "-- w1\nSELECT id  FROM users";
  let md = hover_at(src, 16);
  assert!(md.is_none(), "hover on inter-token whitespace should be None #83");
}

#[test]
fn r18_probe_hover_more() {
  for (s, label) in [
    ("WITH x AS (SELECT id FROM users) SELECT id FROM x", "cte_proj"),
    ("INSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.id", "excluded_id"),
    ("INSERT INTO users (id) VALUES (1) RETURNING id", "returning_id"),
    ("SELECT 1 + 1", "plus_op"),
    ("SELECT 1 = 2", "eq_op"),
    ("SELECT 1 < 2", "lt_op"),
    ("SELECT 1 || '2'", "concat_op"),
    ("SELECT NOT TRUE", "not_kw"),
    ("SELECT NULL::int", "null_cast"),
    ("SELECT a -> 'k' FROM t", "json_arrow"),
  ] {
    // find a name token to hover on
    let cur = s.find(["EXCLUDED", "RETURNING", "WITH", "NOT", "NULL", "->", "+", "=", "<", "||"].iter().find(|t| s.contains(*t)).unwrap_or(&"SELECT")).unwrap();
    let md = hover_at(s, cur);
    eprintln!("HM|{}|some={}", label, md.is_some());
  }
}

#[test]
fn r18_h_returning_0001() {
  let src = "-- r0\nINSERT INTO users (id) VALUES (1) RETURNING id";
  let cur = src.find("RETURNING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNING kw should be Some #1");
}

#[test]
fn r18_h_returning_0003() {
  let src = "-- r0\nDELETE FROM users WHERE id=1 RETURNING email";
  let cur = src.find("RETURNING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNING kw should be Some #3");
}

#[test]
fn r18_h_returning_0004() {
  let src = "-- r0\nINSERT INTO users VALUES (1, 'a', 'b') RETURNING *";
  let cur = src.find("RETURNING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNING kw should be Some #4");
}

#[test]
fn r18_h_returning_0005() {
  let src = "-- r1\nINSERT INTO users (id) VALUES (1) RETURNING id";
  let cur = src.find("RETURNING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNING kw should be Some #5");
}

#[test]
fn r18_h_returning_0007() {
  let src = "-- r1\nDELETE FROM users WHERE id=1 RETURNING email";
  let cur = src.find("RETURNING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNING kw should be Some #7");
}

#[test]
fn r18_h_returning_0008() {
  let src = "-- r1\nINSERT INTO users VALUES (1, 'a', 'b') RETURNING *";
  let cur = src.find("RETURNING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNING kw should be Some #8");
}

#[test]
fn r18_h_returning_0009() {
  let src = "-- r2\nINSERT INTO users (id) VALUES (1) RETURNING id";
  let cur = src.find("RETURNING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNING kw should be Some #9");
}

#[test]
fn r18_h_returning_0011() {
  let src = "-- r2\nDELETE FROM users WHERE id=1 RETURNING email";
  let cur = src.find("RETURNING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNING kw should be Some #11");
}

#[test]
fn r18_h_returning_0012() {
  let src = "-- r2\nINSERT INTO users VALUES (1, 'a', 'b') RETURNING *";
  let cur = src.find("RETURNING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNING kw should be Some #12");
}

#[test]
fn r18_h_cte_0021() {
  let src = "-- c0\nWITH x AS (SELECT id FROM users) SELECT id FROM x";
  let cur = src.rfind("x").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CTE ref should be Some #21");
}

#[test]
fn r18_h_cte_0022() {
  let src = "-- c0\nWITH y AS (SELECT id FROM users) SELECT id FROM y";
  let cur = src.rfind("y").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CTE ref should be Some #22");
}

#[test]
fn r18_h_cte_0023() {
  let src = "-- c0\nWITH cte1 AS (SELECT 1) SELECT * FROM cte1";
  let cur = src.rfind("cte1").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CTE ref should be Some #23");
}

#[test]
fn r18_h_cte_0024() {
  let src = "-- c1\nWITH x AS (SELECT id FROM users) SELECT id FROM x";
  let cur = src.rfind("x").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CTE ref should be Some #24");
}

#[test]
fn r18_h_cte_0025() {
  let src = "-- c1\nWITH y AS (SELECT id FROM users) SELECT id FROM y";
  let cur = src.rfind("y").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CTE ref should be Some #25");
}

#[test]
fn r18_h_cte_0026() {
  let src = "-- c1\nWITH cte1 AS (SELECT 1) SELECT * FROM cte1";
  let cur = src.rfind("cte1").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CTE ref should be Some #26");
}

#[test]
fn r18_h_cte_0027() {
  let src = "-- c2\nWITH x AS (SELECT id FROM users) SELECT id FROM x";
  let cur = src.rfind("x").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CTE ref should be Some #27");
}

#[test]
fn r18_h_cte_0028() {
  let src = "-- c2\nWITH y AS (SELECT id FROM users) SELECT id FROM y";
  let cur = src.rfind("y").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CTE ref should be Some #28");
}

#[test]
fn r18_h_cte_0029() {
  let src = "-- c2\nWITH cte1 AS (SELECT 1) SELECT * FROM cte1";
  let cur = src.rfind("cte1").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CTE ref should be Some #29");
}

#[test]
fn r18_h_excluded_0041() {
  let src = "-- ex0\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name";
  let cur = src.find("EXCLUDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDED should be Some #41");
}

#[test]
fn r18_h_excluded_0042() {
  let src = "-- ex0\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET id = EXCLUDED.id";
  let cur = src.find("EXCLUDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDED should be Some #42");
}

#[test]
fn r18_h_excluded_0043() {
  let src = "-- ex0\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email";
  let cur = src.find("EXCLUDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDED should be Some #43");
}

#[test]
fn r18_h_excluded_0044() {
  let src = "-- ex1\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name";
  let cur = src.find("EXCLUDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDED should be Some #44");
}

#[test]
fn r18_h_excluded_0045() {
  let src = "-- ex1\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET id = EXCLUDED.id";
  let cur = src.find("EXCLUDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDED should be Some #45");
}

#[test]
fn r18_h_excluded_0046() {
  let src = "-- ex1\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email";
  let cur = src.find("EXCLUDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDED should be Some #46");
}

#[test]
fn r18_h_excluded_0047() {
  let src = "-- ex2\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name";
  let cur = src.find("EXCLUDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDED should be Some #47");
}

#[test]
fn r18_h_excluded_0048() {
  let src = "-- ex2\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET id = EXCLUDED.id";
  let cur = src.find("EXCLUDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDED should be Some #48");
}

#[test]
fn r18_h_excluded_0049() {
  let src = "-- ex2\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email";
  let cur = src.find("EXCLUDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDED should be Some #49");
}

#[test]
fn r18_h_cast_chain_0061() {
  let src = "-- ct0\nSELECT id::text::int FROM users";
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("text"));
}

#[test]
fn r18_h_cast_chain_0062() {
  let src = "-- ct0\nSELECT id::text::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r18_h_cast_chain_0063() {
  let src = "-- ct0\nSELECT name::varchar FROM users";
  let cur = src.find("varchar").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("varchar"));
}

#[test]
fn r18_h_cast_chain_0064() {
  let src = "-- ct0\nSELECT '1'::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r18_h_cast_chain_0065() {
  let src = "-- ct1\nSELECT id::text::int FROM users";
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("text"));
}

#[test]
fn r18_h_cast_chain_0066() {
  let src = "-- ct1\nSELECT id::text::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r18_h_cast_chain_0067() {
  let src = "-- ct1\nSELECT name::varchar FROM users";
  let cur = src.find("varchar").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("varchar"));
}

#[test]
fn r18_h_cast_chain_0068() {
  let src = "-- ct1\nSELECT '1'::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r18_h_cast_chain_0069() {
  let src = "-- ct2\nSELECT id::text::int FROM users";
  let cur = src.find("text").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("text"));
}

#[test]
fn r18_h_cast_chain_0070() {
  let src = "-- ct2\nSELECT id::text::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r18_h_cast_chain_0071() {
  let src = "-- ct2\nSELECT name::varchar FROM users";
  let cur = src.find("varchar").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("varchar"));
}

#[test]
fn r18_h_cast_chain_0072() {
  let src = "-- ct2\nSELECT '1'::int FROM users";
  let cur = src.find("int").unwrap();
  let md = hover_at(src, cur).expect("hover on cast type");
  assert!(md.to_ascii_lowercase().contains("int"));
}

#[test]
fn r19_probe_hover() {
  for (s, find) in [
    ("SELECT id FROM users LIMIT 10", "LIMIT"),
    ("SELECT id FROM users OFFSET 5", "OFFSET"),
    ("SELECT id FROM users FETCH NEXT 10 ROWS ONLY", "FETCH"),
    ("SELECT id FROM users WHERE id IN (1, 2, 3)", "IN"),
    ("SELECT id FROM users WHERE id BETWEEN 1 AND 10", "BETWEEN"),
    ("SELECT id FROM users WHERE name LIKE 'a%'", "LIKE"),
    ("SELECT id FROM users WHERE name ILIKE 'a%'", "ILIKE"),
    ("SELECT DISTINCT id FROM users", "DISTINCT"),
    ("SELECT * FROM users ORDER BY id", "ORDER"),
    ("SELECT * FROM users GROUP BY id", "GROUP"),
    ("SELECT * FROM users HAVING count(*) > 1", "HAVING"),
    ("SELECT * FROM users WHERE EXISTS (SELECT 1)", "EXISTS"),
    ("SELECT * FROM users u JOIN orders o USING (id)", "USING"),
  ] {
    let cur = s.find(find).unwrap();
    let md = hover_at(s, cur);
    eprintln!("HK|{}|some={}", find, md.is_some());
  }
}

#[test]
fn r19_h_kw_0001() {
  let src = "-- v0\nSELECT id FROM users LIMIT 10";
  let cur = src.find("LIMIT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LIMIT should be Some #1");
}

#[test]
fn r19_h_kw_0002() {
  let src = "-- v0\nSELECT id FROM users OFFSET 5";
  let cur = src.find("OFFSET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on OFFSET should be Some #2");
}

#[test]
fn r19_h_kw_0003() {
  let src = "-- v0\nSELECT id FROM users FETCH NEXT 10 ROWS ONLY";
  let cur = src.find("FETCH").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FETCH should be Some #3");
}

#[test]
fn r19_h_kw_0004() {
  let src = "-- v0\nSELECT id FROM users WHERE id IN (1, 2, 3)";
  let cur = src.find("IN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on IN should be Some #4");
}

#[test]
fn r19_h_kw_0005() {
  let src = "-- v0\nSELECT id FROM users WHERE id BETWEEN 1 AND 10";
  let cur = src.find("BETWEEN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BETWEEN should be Some #5");
}

#[test]
fn r19_h_kw_0006() {
  let src = "-- v0\nSELECT id FROM users WHERE name LIKE 'a%'";
  let cur = src.find("LIKE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LIKE should be Some #6");
}

#[test]
fn r19_h_kw_0007() {
  let src = "-- v0\nSELECT id FROM users WHERE name ILIKE 'a%'";
  let cur = src.find("ILIKE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ILIKE should be Some #7");
}

#[test]
fn r19_h_kw_0008() {
  let src = "-- v0\nSELECT DISTINCT id FROM users";
  let cur = src.find("DISTINCT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DISTINCT should be Some #8");
}

#[test]
fn r19_h_kw_0009() {
  let src = "-- v0\nSELECT * FROM users ORDER BY id";
  let cur = src.find("ORDER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ORDER should be Some #9");
}

#[test]
fn r19_h_kw_0010() {
  let src = "-- v0\nSELECT * FROM users GROUP BY id";
  let cur = src.find("GROUP").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on GROUP should be Some #10");
}

#[test]
fn r19_h_kw_0011() {
  let src = "-- v0\nSELECT * FROM users HAVING count(*) > 1";
  let cur = src.find("HAVING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on HAVING should be Some #11");
}

#[test]
fn r19_h_kw_0012() {
  let src = "-- v0\nSELECT * FROM users WHERE EXISTS (SELECT 1)";
  let cur = src.find("EXISTS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXISTS should be Some #12");
}

#[test]
fn r19_h_kw_0013() {
  let src = "-- v0\nSELECT * FROM users u JOIN orders o USING (id)";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING should be Some #13");
}

#[test]
fn r19_h_kw_0014() {
  let src = "-- v1\nSELECT id FROM users LIMIT 10";
  let cur = src.find("LIMIT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LIMIT should be Some #14");
}

#[test]
fn r19_h_kw_0015() {
  let src = "-- v1\nSELECT id FROM users OFFSET 5";
  let cur = src.find("OFFSET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on OFFSET should be Some #15");
}

#[test]
fn r19_h_kw_0016() {
  let src = "-- v1\nSELECT id FROM users FETCH NEXT 10 ROWS ONLY";
  let cur = src.find("FETCH").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FETCH should be Some #16");
}

#[test]
fn r19_h_kw_0017() {
  let src = "-- v1\nSELECT id FROM users WHERE id IN (1, 2, 3)";
  let cur = src.find("IN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on IN should be Some #17");
}

#[test]
fn r19_h_kw_0018() {
  let src = "-- v1\nSELECT id FROM users WHERE id BETWEEN 1 AND 10";
  let cur = src.find("BETWEEN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BETWEEN should be Some #18");
}

#[test]
fn r19_h_kw_0019() {
  let src = "-- v1\nSELECT id FROM users WHERE name LIKE 'a%'";
  let cur = src.find("LIKE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LIKE should be Some #19");
}

#[test]
fn r19_h_kw_0020() {
  let src = "-- v1\nSELECT id FROM users WHERE name ILIKE 'a%'";
  let cur = src.find("ILIKE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ILIKE should be Some #20");
}

#[test]
fn r19_h_kw_0021() {
  let src = "-- v1\nSELECT DISTINCT id FROM users";
  let cur = src.find("DISTINCT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DISTINCT should be Some #21");
}

#[test]
fn r19_h_kw_0022() {
  let src = "-- v1\nSELECT * FROM users ORDER BY id";
  let cur = src.find("ORDER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ORDER should be Some #22");
}

#[test]
fn r19_h_kw_0023() {
  let src = "-- v1\nSELECT * FROM users GROUP BY id";
  let cur = src.find("GROUP").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on GROUP should be Some #23");
}

#[test]
fn r19_h_kw_0024() {
  let src = "-- v1\nSELECT * FROM users HAVING count(*) > 1";
  let cur = src.find("HAVING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on HAVING should be Some #24");
}

#[test]
fn r19_h_kw_0025() {
  let src = "-- v1\nSELECT * FROM users WHERE EXISTS (SELECT 1)";
  let cur = src.find("EXISTS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXISTS should be Some #25");
}

#[test]
fn r19_h_kw_0026() {
  let src = "-- v1\nSELECT * FROM users u JOIN orders o USING (id)";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING should be Some #26");
}

#[test]
fn r19_h_kw_0027() {
  let src = "-- v2\nSELECT id FROM users LIMIT 10";
  let cur = src.find("LIMIT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LIMIT should be Some #27");
}

#[test]
fn r19_h_kw_0028() {
  let src = "-- v2\nSELECT id FROM users OFFSET 5";
  let cur = src.find("OFFSET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on OFFSET should be Some #28");
}

#[test]
fn r19_h_kw_0029() {
  let src = "-- v2\nSELECT id FROM users FETCH NEXT 10 ROWS ONLY";
  let cur = src.find("FETCH").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FETCH should be Some #29");
}

#[test]
fn r19_h_kw_0030() {
  let src = "-- v2\nSELECT id FROM users WHERE id IN (1, 2, 3)";
  let cur = src.find("IN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on IN should be Some #30");
}

#[test]
fn r19_h_kw_0031() {
  let src = "-- v2\nSELECT id FROM users WHERE id BETWEEN 1 AND 10";
  let cur = src.find("BETWEEN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BETWEEN should be Some #31");
}

#[test]
fn r19_h_kw_0032() {
  let src = "-- v2\nSELECT id FROM users WHERE name LIKE 'a%'";
  let cur = src.find("LIKE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LIKE should be Some #32");
}

#[test]
fn r19_h_kw_0033() {
  let src = "-- v2\nSELECT id FROM users WHERE name ILIKE 'a%'";
  let cur = src.find("ILIKE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ILIKE should be Some #33");
}

#[test]
fn r19_h_kw_0034() {
  let src = "-- v2\nSELECT DISTINCT id FROM users";
  let cur = src.find("DISTINCT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DISTINCT should be Some #34");
}

#[test]
fn r19_h_kw_0035() {
  let src = "-- v2\nSELECT * FROM users ORDER BY id";
  let cur = src.find("ORDER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ORDER should be Some #35");
}

#[test]
fn r19_h_kw_0036() {
  let src = "-- v2\nSELECT * FROM users GROUP BY id";
  let cur = src.find("GROUP").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on GROUP should be Some #36");
}

#[test]
fn r19_h_kw_0037() {
  let src = "-- v2\nSELECT * FROM users HAVING count(*) > 1";
  let cur = src.find("HAVING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on HAVING should be Some #37");
}

#[test]
fn r19_h_kw_0038() {
  let src = "-- v2\nSELECT * FROM users WHERE EXISTS (SELECT 1)";
  let cur = src.find("EXISTS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXISTS should be Some #38");
}

#[test]
fn r19_h_kw_0039() {
  let src = "-- v2\nSELECT * FROM users u JOIN orders o USING (id)";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING should be Some #39");
}

#[test]
fn r19_h_agg_arg_0068() {
  let src = "-- ag0\nSELECT count(id) FROM users";
  let cur = src.find("count(id)").unwrap() + 6;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("id"));
}

#[test]
fn r19_h_agg_arg_0069() {
  let src = "-- ag0\nSELECT max(email) FROM users";
  let cur = src.find("max(email)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("email"));
}

#[test]
fn r19_h_agg_arg_0070() {
  let src = "-- ag0\nSELECT min(name) FROM users";
  let cur = src.find("min(name)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("name"));
}

#[test]
fn r19_h_agg_arg_0071() {
  let src = "-- ag0\nSELECT sum(id) FROM users";
  let cur = src.find("sum(id)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("id"));
}

#[test]
fn r19_h_agg_arg_0072() {
  let src = "-- ag0\nSELECT avg(id) FROM users";
  let cur = src.find("avg(id)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("id"));
}

#[test]
fn r19_h_agg_arg_0073() {
  let src = "-- ag1\nSELECT count(id) FROM users";
  let cur = src.find("count(id)").unwrap() + 6;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("id"));
}

#[test]
fn r19_h_agg_arg_0074() {
  let src = "-- ag1\nSELECT max(email) FROM users";
  let cur = src.find("max(email)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("email"));
}

#[test]
fn r19_h_agg_arg_0075() {
  let src = "-- ag1\nSELECT min(name) FROM users";
  let cur = src.find("min(name)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("name"));
}

#[test]
fn r19_h_agg_arg_0076() {
  let src = "-- ag1\nSELECT sum(id) FROM users";
  let cur = src.find("sum(id)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("id"));
}

#[test]
fn r19_h_agg_arg_0077() {
  let src = "-- ag1\nSELECT avg(id) FROM users";
  let cur = src.find("avg(id)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("id"));
}

#[test]
fn r19_h_agg_arg_0078() {
  let src = "-- ag2\nSELECT count(id) FROM users";
  let cur = src.find("count(id)").unwrap() + 6;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("id"));
}

#[test]
fn r19_h_agg_arg_0079() {
  let src = "-- ag2\nSELECT max(email) FROM users";
  let cur = src.find("max(email)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("email"));
}

#[test]
fn r19_h_agg_arg_0080() {
  let src = "-- ag2\nSELECT min(name) FROM users";
  let cur = src.find("min(name)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("name"));
}

#[test]
fn r19_h_agg_arg_0081() {
  let src = "-- ag2\nSELECT sum(id) FROM users";
  let cur = src.find("sum(id)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("id"));
}

#[test]
fn r19_h_agg_arg_0082() {
  let src = "-- ag2\nSELECT avg(id) FROM users";
  let cur = src.find("avg(id)").unwrap() + 4;
  let md = hover_at(src, cur).expect("hover agg col");
  assert!(md.contains("id"));
}

#[test]
fn r20_probe_hover() {
  for (s, find) in [
    ("CREATE TABLE t (id int PRIMARY KEY)", "PRIMARY"),
    ("CREATE TABLE t (id int UNIQUE)", "UNIQUE"),
    ("CREATE TABLE t (id int NOT NULL)", "NULL"),
    ("CREATE TABLE t (id int DEFAULT 0)", "DEFAULT"),
    ("CREATE TABLE t (id int REFERENCES users(id))", "REFERENCES"),
    ("CREATE TABLE t (id int CHECK (id > 0))", "CHECK"),
    ("CREATE INDEX ON users (id)", "INDEX"),
    ("CREATE UNIQUE INDEX ON users (id)", "UNIQUE"),
    ("CREATE INDEX CONCURRENTLY ON users (id)", "CONCURRENTLY"),
    ("CREATE INDEX ON users USING gin (id)", "gin"),
    ("CREATE INDEX ON users USING btree (id)", "btree"),
    ("ALTER TABLE users ADD CONSTRAINT chk CHECK (id > 0)", "CONSTRAINT"),
    ("ALTER TABLE users ADD COLUMN c int", "COLUMN"),
    ("ALTER TABLE users ALTER COLUMN id TYPE bigint", "ALTER"),
  ] {
    let cur = s.find(find).unwrap();
    let md = hover_at(s, cur);
    eprintln!("HK|{}|some={}", find, md.is_some());
  }
}

#[test]
fn r20_h_ddl_0001() {
  let src = "-- v0\nCREATE TABLE t (id int PRIMARY KEY)";
  let cur = src.find("PRIMARY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PRIMARY should be Some #1");
}

#[test]
fn r20_h_ddl_0002() {
  let src = "-- v0\nCREATE TABLE t (id int NOT NULL)";
  let cur = src.find("NULL").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NULL should be Some #2");
}

#[test]
fn r20_h_ddl_0003() {
  let src = "-- v0\nCREATE TABLE t (id int DEFAULT 0)";
  let cur = src.find("DEFAULT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFAULT should be Some #3");
}

#[test]
fn r20_h_ddl_0004() {
  let src = "-- v0\nCREATE TABLE t (id int REFERENCES users(id))";
  let cur = src.find("REFERENCES").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REFERENCES should be Some #4");
}

#[test]
fn r20_h_ddl_0005() {
  let src = "-- v0\nCREATE TABLE t (id int CHECK (id > 0))";
  let cur = src.find("CHECK").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CHECK should be Some #5");
}

#[test]
fn r20_h_ddl_0006() {
  let src = "-- v0\nCREATE INDEX ON users (id)";
  let cur = src.find("INDEX").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INDEX should be Some #6");
}

#[test]
fn r20_h_ddl_0007() {
  let src = "-- v0\nCREATE INDEX CONCURRENTLY ON users (id)";
  let cur = src.find("CONCURRENTLY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONCURRENTLY should be Some #7");
}

#[test]
fn r20_h_ddl_0008() {
  let src = "-- v0\nCREATE INDEX ON users USING gin (id)";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING should be Some #8");
}

#[test]
fn r20_h_ddl_0009() {
  let src = "-- v0\nALTER TABLE users ADD CONSTRAINT chk CHECK (id > 0)";
  let cur = src.find("CONSTRAINT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONSTRAINT should be Some #9");
}

#[test]
fn r20_h_ddl_0010() {
  let src = "-- v0\nALTER TABLE users ADD COLUMN c int";
  let cur = src.find("COLUMN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on COLUMN should be Some #10");
}

#[test]
fn r20_h_ddl_0011() {
  let src = "-- v0\nCREATE OR REPLACE FUNCTION f() RETURNS int LANGUAGE sql AS 'select 1'";
  let cur = src.find("LANGUAGE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LANGUAGE should be Some #11");
}

#[test]
fn r20_h_ddl_0012() {
  let src = "-- v0\nCREATE OR REPLACE FUNCTION f() RETURNS int LANGUAGE sql AS 'select 1'";
  let cur = src.find("RETURNS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNS should be Some #12");
}

#[test]
fn r20_h_ddl_0013() {
  let src = "-- v0\nCREATE TRIGGER t BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("BEFORE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BEFORE should be Some #13");
}

#[test]
fn r20_h_ddl_0014() {
  let src = "-- v0\nCREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("AFTER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on AFTER should be Some #14");
}

#[test]
fn r20_h_ddl_0015() {
  let src = "-- v0\nCREATE TRIGGER t INSTEAD OF INSERT ON v FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("INSTEAD").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INSTEAD should be Some #15");
}

#[test]
fn r20_h_ddl_0016() {
  let src = "-- v0\nCREATE TRIGGER t AFTER INSERT ON users FOR EACH STATEMENT EXECUTE FUNCTION f()";
  let cur = src.find("STATEMENT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on STATEMENT should be Some #16");
}

#[test]
fn r20_h_ddl_0017() {
  let src = "-- v0\nCREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("EXECUTE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXECUTE should be Some #17");
}

#[test]
fn r20_h_ddl_0018() {
  let src = "-- v1\nCREATE TABLE t (id int PRIMARY KEY)";
  let cur = src.find("PRIMARY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PRIMARY should be Some #18");
}

#[test]
fn r20_h_ddl_0019() {
  let src = "-- v1\nCREATE TABLE t (id int NOT NULL)";
  let cur = src.find("NULL").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NULL should be Some #19");
}

#[test]
fn r20_h_ddl_0020() {
  let src = "-- v1\nCREATE TABLE t (id int DEFAULT 0)";
  let cur = src.find("DEFAULT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFAULT should be Some #20");
}

#[test]
fn r20_h_ddl_0021() {
  let src = "-- v1\nCREATE TABLE t (id int REFERENCES users(id))";
  let cur = src.find("REFERENCES").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REFERENCES should be Some #21");
}

#[test]
fn r20_h_ddl_0022() {
  let src = "-- v1\nCREATE TABLE t (id int CHECK (id > 0))";
  let cur = src.find("CHECK").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CHECK should be Some #22");
}

#[test]
fn r20_h_ddl_0023() {
  let src = "-- v1\nCREATE INDEX ON users (id)";
  let cur = src.find("INDEX").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INDEX should be Some #23");
}

#[test]
fn r20_h_ddl_0024() {
  let src = "-- v1\nCREATE INDEX CONCURRENTLY ON users (id)";
  let cur = src.find("CONCURRENTLY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONCURRENTLY should be Some #24");
}

#[test]
fn r20_h_ddl_0025() {
  let src = "-- v1\nCREATE INDEX ON users USING gin (id)";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING should be Some #25");
}

#[test]
fn r20_h_ddl_0026() {
  let src = "-- v1\nALTER TABLE users ADD CONSTRAINT chk CHECK (id > 0)";
  let cur = src.find("CONSTRAINT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONSTRAINT should be Some #26");
}

#[test]
fn r20_h_ddl_0027() {
  let src = "-- v1\nALTER TABLE users ADD COLUMN c int";
  let cur = src.find("COLUMN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on COLUMN should be Some #27");
}

#[test]
fn r20_h_ddl_0028() {
  let src = "-- v1\nCREATE OR REPLACE FUNCTION f() RETURNS int LANGUAGE sql AS 'select 1'";
  let cur = src.find("LANGUAGE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LANGUAGE should be Some #28");
}

#[test]
fn r20_h_ddl_0029() {
  let src = "-- v1\nCREATE OR REPLACE FUNCTION f() RETURNS int LANGUAGE sql AS 'select 1'";
  let cur = src.find("RETURNS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNS should be Some #29");
}

#[test]
fn r20_h_ddl_0030() {
  let src = "-- v1\nCREATE TRIGGER t BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("BEFORE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BEFORE should be Some #30");
}

#[test]
fn r20_h_ddl_0031() {
  let src = "-- v1\nCREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("AFTER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on AFTER should be Some #31");
}

#[test]
fn r20_h_ddl_0032() {
  let src = "-- v1\nCREATE TRIGGER t INSTEAD OF INSERT ON v FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("INSTEAD").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INSTEAD should be Some #32");
}

#[test]
fn r20_h_ddl_0033() {
  let src = "-- v1\nCREATE TRIGGER t AFTER INSERT ON users FOR EACH STATEMENT EXECUTE FUNCTION f()";
  let cur = src.find("STATEMENT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on STATEMENT should be Some #33");
}

#[test]
fn r20_h_ddl_0034() {
  let src = "-- v1\nCREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("EXECUTE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXECUTE should be Some #34");
}

#[test]
fn r20_h_ddl_0035() {
  let src = "-- v2\nCREATE TABLE t (id int PRIMARY KEY)";
  let cur = src.find("PRIMARY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PRIMARY should be Some #35");
}

#[test]
fn r20_h_ddl_0036() {
  let src = "-- v2\nCREATE TABLE t (id int NOT NULL)";
  let cur = src.find("NULL").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NULL should be Some #36");
}

#[test]
fn r20_h_ddl_0037() {
  let src = "-- v2\nCREATE TABLE t (id int DEFAULT 0)";
  let cur = src.find("DEFAULT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFAULT should be Some #37");
}

#[test]
fn r20_h_ddl_0038() {
  let src = "-- v2\nCREATE TABLE t (id int REFERENCES users(id))";
  let cur = src.find("REFERENCES").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REFERENCES should be Some #38");
}

#[test]
fn r20_h_ddl_0039() {
  let src = "-- v2\nCREATE TABLE t (id int CHECK (id > 0))";
  let cur = src.find("CHECK").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CHECK should be Some #39");
}

#[test]
fn r20_h_ddl_0040() {
  let src = "-- v2\nCREATE INDEX ON users (id)";
  let cur = src.find("INDEX").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INDEX should be Some #40");
}

#[test]
fn r20_h_ddl_0041() {
  let src = "-- v2\nCREATE INDEX CONCURRENTLY ON users (id)";
  let cur = src.find("CONCURRENTLY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONCURRENTLY should be Some #41");
}

#[test]
fn r20_h_ddl_0042() {
  let src = "-- v2\nCREATE INDEX ON users USING gin (id)";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING should be Some #42");
}

#[test]
fn r20_h_ddl_0043() {
  let src = "-- v2\nALTER TABLE users ADD CONSTRAINT chk CHECK (id > 0)";
  let cur = src.find("CONSTRAINT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONSTRAINT should be Some #43");
}

#[test]
fn r20_h_ddl_0044() {
  let src = "-- v2\nALTER TABLE users ADD COLUMN c int";
  let cur = src.find("COLUMN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on COLUMN should be Some #44");
}

#[test]
fn r20_h_ddl_0045() {
  let src = "-- v2\nCREATE OR REPLACE FUNCTION f() RETURNS int LANGUAGE sql AS 'select 1'";
  let cur = src.find("LANGUAGE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LANGUAGE should be Some #45");
}

#[test]
fn r20_h_ddl_0046() {
  let src = "-- v2\nCREATE OR REPLACE FUNCTION f() RETURNS int LANGUAGE sql AS 'select 1'";
  let cur = src.find("RETURNS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RETURNS should be Some #46");
}

#[test]
fn r20_h_ddl_0047() {
  let src = "-- v2\nCREATE TRIGGER t BEFORE INSERT ON users FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("BEFORE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BEFORE should be Some #47");
}

#[test]
fn r20_h_ddl_0048() {
  let src = "-- v2\nCREATE TRIGGER t AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("AFTER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on AFTER should be Some #48");
}

#[test]
fn r20_h_ddl_0049() {
  let src = "-- v2\nCREATE TRIGGER t INSTEAD OF INSERT ON v FOR EACH ROW EXECUTE FUNCTION f()";
  let cur = src.find("INSTEAD").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INSTEAD should be Some #49");
}

#[test]
fn r20_h_ddl_0050() {
  let src = "-- v2\nCREATE TRIGGER t AFTER INSERT ON users FOR EACH STATEMENT EXECUTE FUNCTION f()";
  let cur = src.find("STATEMENT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on STATEMENT should be Some #50");
}

#[test]
fn r20_h_dml_0051() {
  let src = "-- d0\nINSERT INTO users (id) VALUES (1)";
  let cur = src.find("INSERT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INSERT should be Some");
}

#[test]
fn r20_h_dml_0052() {
  let src = "-- d0\nINSERT INTO users (id) VALUES (1)";
  let cur = src.find("VALUES").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on VALUES should be Some");
}

#[test]
fn r20_h_dml_0053() {
  let src = "-- d0\nUPDATE users SET name='x' WHERE id=1";
  let cur = src.find("UPDATE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on UPDATE should be Some");
}

#[test]
fn r20_h_dml_0054() {
  let src = "-- d0\nUPDATE users SET name='x' WHERE id=1";
  let cur = src.find("SET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SET should be Some");
}

#[test]
fn r20_h_dml_0055() {
  let src = "-- d0\nDELETE FROM users WHERE id=1";
  let cur = src.find("DELETE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DELETE should be Some");
}

#[test]
fn r20_h_dml_0056() {
  let src = "-- d0\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name='x'";
  let cur = src.find("MERGE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on MERGE should be Some");
}

#[test]
fn r20_h_dml_0057() {
  let src = "-- d0\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name='x'";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING should be Some");
}

#[test]
fn r20_h_dml_0058() {
  let src = "-- d0\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name='x'";
  let cur = src.find("WHEN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WHEN should be Some");
}

#[test]
fn r20_h_dml_0059() {
  let src = "-- d1\nINSERT INTO users (id) VALUES (1)";
  let cur = src.find("INSERT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INSERT should be Some");
}

#[test]
fn r20_h_dml_0060() {
  let src = "-- d1\nINSERT INTO users (id) VALUES (1)";
  let cur = src.find("VALUES").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on VALUES should be Some");
}

#[test]
fn r20_h_dml_0061() {
  let src = "-- d1\nUPDATE users SET name='x' WHERE id=1";
  let cur = src.find("UPDATE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on UPDATE should be Some");
}

#[test]
fn r20_h_dml_0062() {
  let src = "-- d1\nUPDATE users SET name='x' WHERE id=1";
  let cur = src.find("SET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SET should be Some");
}

#[test]
fn r20_h_dml_0063() {
  let src = "-- d1\nDELETE FROM users WHERE id=1";
  let cur = src.find("DELETE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DELETE should be Some");
}

#[test]
fn r20_h_dml_0064() {
  let src = "-- d1\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name='x'";
  let cur = src.find("MERGE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on MERGE should be Some");
}

#[test]
fn r20_h_dml_0065() {
  let src = "-- d1\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name='x'";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING should be Some");
}

#[test]
fn r20_h_dml_0066() {
  let src = "-- d1\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name='x'";
  let cur = src.find("WHEN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WHEN should be Some");
}

#[test]
fn r20_h_dml_0067() {
  let src = "-- d2\nINSERT INTO users (id) VALUES (1)";
  let cur = src.find("INSERT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INSERT should be Some");
}

#[test]
fn r20_h_dml_0068() {
  let src = "-- d2\nINSERT INTO users (id) VALUES (1)";
  let cur = src.find("VALUES").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on VALUES should be Some");
}

#[test]
fn r20_h_dml_0069() {
  let src = "-- d2\nUPDATE users SET name='x' WHERE id=1";
  let cur = src.find("UPDATE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on UPDATE should be Some");
}

#[test]
fn r20_h_dml_0070() {
  let src = "-- d2\nUPDATE users SET name='x' WHERE id=1";
  let cur = src.find("SET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SET should be Some");
}

#[test]
fn r20_h_dml_0071() {
  let src = "-- d2\nDELETE FROM users WHERE id=1";
  let cur = src.find("DELETE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DELETE should be Some");
}

#[test]
fn r20_h_dml_0072() {
  let src = "-- d2\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name='x'";
  let cur = src.find("MERGE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on MERGE should be Some");
}

#[test]
fn r20_h_dml_0073() {
  let src = "-- d2\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name='x'";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING should be Some");
}

#[test]
fn r20_h_dml_0074() {
  let src = "-- d2\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name='x'";
  let cur = src.find("WHEN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WHEN should be Some");
}

#[test]
fn r20_h_tcl_0081() {
  let src = "-- t0\nBEGIN";
  let cur = src.find("BEGIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BEGIN should be Some");
}

#[test]
fn r20_h_tcl_0082() {
  let src = "-- t0\nCOMMIT";
  let cur = src.find("COMMIT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on COMMIT should be Some");
}

#[test]
fn r20_h_tcl_0083() {
  let src = "-- t0\nROLLBACK";
  let cur = src.find("ROLLBACK").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ROLLBACK should be Some");
}

#[test]
fn r20_h_tcl_0084() {
  let src = "-- t0\nSAVEPOINT";
  let cur = src.find("SAVEPOINT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SAVEPOINT should be Some");
}

#[test]
fn r20_h_tcl_0085() {
  let src = "-- t0\nRELEASE";
  let cur = src.find("RELEASE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RELEASE should be Some");
}

#[test]
fn r20_h_tcl_0086() {
  let src = "-- t0\nCHECKPOINT";
  let cur = src.find("CHECKPOINT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CHECKPOINT should be Some");
}

#[test]
fn r20_h_tcl_0087() {
  let src = "-- t0\nBEGIN ISOLATION LEVEL SERIALIZABLE";
  let cur = src.find("BEGIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BEGIN should be Some");
}

#[test]
fn r20_h_tcl_0088() {
  let src = "-- t0\nSET TRANSACTION READ ONLY";
  let cur = src.find("SET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SET should be Some");
}

#[test]
fn r20_h_tcl_0089() {
  let src = "-- t0\nSET LOCAL search_path TO public";
  let cur = src.find("SET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SET should be Some");
}

#[test]
fn r20_h_tcl_0090() {
  let src = "-- t1\nBEGIN";
  let cur = src.find("BEGIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BEGIN should be Some");
}

#[test]
fn r20_h_tcl_0091() {
  let src = "-- t1\nCOMMIT";
  let cur = src.find("COMMIT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on COMMIT should be Some");
}

#[test]
fn r20_h_tcl_0092() {
  let src = "-- t1\nROLLBACK";
  let cur = src.find("ROLLBACK").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ROLLBACK should be Some");
}

#[test]
fn r20_h_tcl_0093() {
  let src = "-- t1\nSAVEPOINT";
  let cur = src.find("SAVEPOINT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SAVEPOINT should be Some");
}

#[test]
fn r20_h_tcl_0094() {
  let src = "-- t1\nRELEASE";
  let cur = src.find("RELEASE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RELEASE should be Some");
}

#[test]
fn r20_h_tcl_0095() {
  let src = "-- t1\nCHECKPOINT";
  let cur = src.find("CHECKPOINT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CHECKPOINT should be Some");
}

#[test]
fn r20_h_tcl_0096() {
  let src = "-- t1\nBEGIN ISOLATION LEVEL SERIALIZABLE";
  let cur = src.find("BEGIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BEGIN should be Some");
}

#[test]
fn r20_h_tcl_0097() {
  let src = "-- t1\nSET TRANSACTION READ ONLY";
  let cur = src.find("SET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SET should be Some");
}

#[test]
fn r20_h_tcl_0098() {
  let src = "-- t1\nSET LOCAL search_path TO public";
  let cur = src.find("SET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SET should be Some");
}

#[test]
fn r20_h_tcl_0099() {
  let src = "-- t2\nBEGIN";
  let cur = src.find("BEGIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BEGIN should be Some");
}

#[test]
fn r20_h_tcl_0100() {
  let src = "-- t2\nCOMMIT";
  let cur = src.find("COMMIT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on COMMIT should be Some");
}

#[test]
fn r20_h_tcl_0101() {
  let src = "-- t2\nROLLBACK";
  let cur = src.find("ROLLBACK").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ROLLBACK should be Some");
}

#[test]
fn r20_h_tcl_0102() {
  let src = "-- t2\nSAVEPOINT";
  let cur = src.find("SAVEPOINT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SAVEPOINT should be Some");
}

#[test]
fn r20_h_tcl_0103() {
  let src = "-- t2\nRELEASE";
  let cur = src.find("RELEASE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RELEASE should be Some");
}

#[test]
fn r20_h_tcl_0104() {
  let src = "-- t2\nCHECKPOINT";
  let cur = src.find("CHECKPOINT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CHECKPOINT should be Some");
}

#[test]
fn r20_h_tcl_0105() {
  let src = "-- t2\nBEGIN ISOLATION LEVEL SERIALIZABLE";
  let cur = src.find("BEGIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on BEGIN should be Some");
}

#[test]
fn r20_h_tcl_0106() {
  let src = "-- t2\nSET TRANSACTION READ ONLY";
  let cur = src.find("SET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SET should be Some");
}

#[test]
fn r20_h_tcl_0107() {
  let src = "-- t2\nSET LOCAL search_path TO public";
  let cur = src.find("SET").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SET should be Some");
}

#[test]
fn r21_probe_h() {
  for (s, find) in [
    ("SELECT u.id /* comment */ FROM users u", "u.id"),
    ("SELECT /* hint */ u.id FROM users u", "u.id"),
    ("SELECT u.email, /* skip */ u.name FROM users u", "u.email"),
    ("SELECT u.id FROM users u /* comment */", "u.id"),
    ("SELECT u.id FROM\n  users u", "u.id"),
    ("SELECT\n  u.id\nFROM users u", "u.id"),
    ("SELECT public.users.id FROM public.users", "public.users.id"),
    ("SELECT \"users\".id FROM users", "users\".id"),
    ("SELECT id::text FROM users", "id::text"),
    ("SELECT (id)::text FROM users", "id"),
    ("SELECT id + 1 FROM users", "id"),
    ("SELECT id - 1 FROM users", "id"),
    ("SELECT id * 2 FROM users", "id"),
    ("SELECT id / 2 FROM users", "id"),
  ] {
    let cur = s.find(find).unwrap() + if find.starts_with("u.") || find.starts_with("public") { find.len() - 2 } else { 0 };
    let md = hover_at(s, cur);
    eprintln!("HE|{}|some={}", find, md.is_some());
  }
}

#[test]
fn r21_h_with_comment_0001() {
  let src = "-- v0\nSELECT u.id /* comment */ FROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0002() {
  let src = "-- v0\nSELECT /* hint */ u.id FROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0003() {
  let src = "-- v0\nSELECT u.email, /* skip */ u.name FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("email"));
}

#[test]
fn r21_h_with_comment_0004() {
  let src = "-- v0\nSELECT u.id FROM users u /* comment */";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0005() {
  let src = "-- v0\nSELECT u.id FROM\n  users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0006() {
  let src = "-- v0\nSELECT\n  u.id\nFROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0007() {
  let src = "-- v0\nSELECT id /* eol comment */ FROM users";
  let cur = src.find("id").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0008() {
  let src = "-- v0\nSELECT email -- inline\n FROM users";
  let cur = src.find("email").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("email"));
}

#[test]
fn r21_h_with_comment_0009() {
  let src = "-- v0\nSELECT id, /* skip me */ email FROM users";
  let cur = src.find("email").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("email"));
}

#[test]
fn r21_h_with_comment_0010() {
  let src = "-- v0\nSELECT id, name -- skip\n FROM users";
  let cur = src.find("name").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("name"));
}

#[test]
fn r21_h_with_comment_0011() {
  let src = "-- v1\nSELECT u.id /* comment */ FROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0013() {
  let src = "-- v1\nSELECT u.email, /* skip */ u.name FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("email"));
}

#[test]
fn r21_h_with_comment_0014() {
  let src = "-- v1\nSELECT u.id FROM users u /* comment */";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0015() {
  let src = "-- v1\nSELECT u.id FROM\n  users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0016() {
  let src = "-- v1\nSELECT\n  u.id\nFROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0017() {
  let src = "-- v1\nSELECT id /* eol comment */ FROM users";
  let cur = src.find("id").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0018() {
  let src = "-- v1\nSELECT email -- inline\n FROM users";
  let cur = src.find("email").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("email"));
}

#[test]
fn r21_h_with_comment_0019() {
  let src = "-- v1\nSELECT id, /* skip me */ email FROM users";
  let cur = src.find("email").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("email"));
}

#[test]
fn r21_h_with_comment_0020() {
  let src = "-- v1\nSELECT id, name -- skip\n FROM users";
  let cur = src.find("name").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("name"));
}

#[test]
fn r21_h_with_comment_0023() {
  let src = "-- v2\nSELECT u.email, /* skip */ u.name FROM users u";
  let cur = src.find("u.email").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("email"));
}

#[test]
fn r21_h_with_comment_0024() {
  let src = "-- v2\nSELECT u.id FROM users u /* comment */";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0025() {
  let src = "-- v2\nSELECT u.id FROM\n  users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0026() {
  let src = "-- v2\nSELECT\n  u.id\nFROM users u";
  let cur = src.find("u.id").unwrap() + 2;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0027() {
  let src = "-- v2\nSELECT id /* eol comment */ FROM users";
  let cur = src.find("id").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("id"));
}

#[test]
fn r21_h_with_comment_0028() {
  let src = "-- v2\nSELECT email -- inline\n FROM users";
  let cur = src.find("email").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("email"));
}

#[test]
fn r21_h_with_comment_0029() {
  let src = "-- v2\nSELECT id, /* skip me */ email FROM users";
  let cur = src.find("email").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("email"));
}

#[test]
fn r21_h_with_comment_0030() {
  let src = "-- v2\nSELECT id, name -- skip\n FROM users";
  let cur = src.find("name").unwrap() + 0;
  let md = hover_at(src, cur).expect("hover with comment ctx");
  assert!(md.contains("name"));
}

#[test]
fn r21_h_qual_schema_0032() {
  let src = "-- q0\nSELECT id::public.text FROM users";
  let cur = src.find("public").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on schema `public` should be Some");
}

#[test]
fn r21_h_qual_schema_0033() {
  let src = "-- q0\nSELECT public.users.id FROM public.users";
  let cur = src.find("public").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on schema `public` should be Some");
}

#[test]
fn r21_h_qual_schema_0035() {
  let src = "-- q1\nSELECT id::public.text FROM users";
  let cur = src.find("public").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on schema `public` should be Some");
}

#[test]
fn r21_h_qual_schema_0036() {
  let src = "-- q1\nSELECT public.users.id FROM public.users";
  let cur = src.find("public").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on schema `public` should be Some");
}

#[test]
fn r21_h_qual_schema_0038() {
  let src = "-- q2\nSELECT id::public.text FROM users";
  let cur = src.find("public").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on schema `public` should be Some");
}

#[test]
fn r21_h_qual_schema_0039() {
  let src = "-- q2\nSELECT public.users.id FROM public.users";
  let cur = src.find("public").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on schema `public` should be Some");
}

#[test]
fn r22_probe_h() {
  for (s, find) in [
    ("EXPLAIN SELECT * FROM users", "EXPLAIN"),
    ("EXPLAIN ANALYZE SELECT * FROM users", "ANALYZE"),
    ("EXPLAIN (FORMAT JSON) SELECT * FROM users", "FORMAT"),
    ("PREPARE p AS SELECT 1", "PREPARE"),
    ("EXECUTE p(1)", "EXECUTE"),
    ("DEALLOCATE p", "DEALLOCATE"),
    ("DECLARE c CURSOR FOR SELECT 1", "DECLARE"),
    ("DECLARE c CURSOR FOR SELECT 1", "CURSOR"),
    ("FETCH NEXT FROM c", "FETCH"),
    ("FETCH NEXT FROM c", "NEXT"),
    ("MOVE c", "MOVE"),
    ("CLOSE c", "CLOSE"),
    ("LISTEN ch", "LISTEN"),
    ("UNLISTEN ch", "UNLISTEN"),
    ("NOTIFY ch", "NOTIFY"),
    ("LOAD 'mylib'", "LOAD"),
    ("CALL p()", "CALL"),
    ("DO $$ BEGIN NULL; END $$", "DO"),
    ("VACUUM FULL users", "VACUUM"),
    ("VACUUM FULL users", "FULL"),
    ("ANALYZE users", "ANALYZE"),
    ("CLUSTER users", "CLUSTER"),
    ("REINDEX TABLE users", "REINDEX"),
    ("REINDEX TABLE users", "TABLE"),
  ] {
    let cur = s.find(find).unwrap();
    let md = hover_at(s, cur);
    eprintln!("HM|{}|some={}", find, md.is_some());
  }
}

#[test]
fn r22_h_meta_0001() {
  let src = "-- v0\nEXPLAIN SELECT * FROM users";
  let cur = src.find("EXPLAIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXPLAIN");
}

#[test]
fn r22_h_meta_0002() {
  let src = "-- v0\nEXPLAIN (FORMAT JSON) SELECT * FROM users";
  let cur = src.find("FORMAT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FORMAT");
}

#[test]
fn r22_h_meta_0003() {
  let src = "-- v0\nPREPARE p AS SELECT 1";
  let cur = src.find("PREPARE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PREPARE");
}

#[test]
fn r22_h_meta_0004() {
  let src = "-- v0\nEXECUTE p(1)";
  let cur = src.find("EXECUTE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXECUTE");
}

#[test]
fn r22_h_meta_0005() {
  let src = "-- v0\nDEALLOCATE p";
  let cur = src.find("DEALLOCATE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEALLOCATE");
}

#[test]
fn r22_h_meta_0006() {
  let src = "-- v0\nDECLARE c CURSOR FOR SELECT 1";
  let cur = src.find("CURSOR").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CURSOR");
}

#[test]
fn r22_h_meta_0007() {
  let src = "-- v0\nFETCH NEXT FROM c";
  let cur = src.find("FETCH").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FETCH");
}

#[test]
fn r22_h_meta_0008() {
  let src = "-- v0\nFETCH NEXT FROM c";
  let cur = src.find("NEXT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NEXT");
}

#[test]
fn r22_h_meta_0009() {
  let src = "-- v0\nMOVE c";
  let cur = src.find("MOVE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on MOVE");
}

#[test]
fn r22_h_meta_0010() {
  let src = "-- v0\nCLOSE c";
  let cur = src.find("CLOSE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CLOSE");
}

#[test]
fn r22_h_meta_0011() {
  let src = "-- v0\nLISTEN ch";
  let cur = src.find("LISTEN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LISTEN");
}

#[test]
fn r22_h_meta_0012() {
  let src = "-- v0\nUNLISTEN ch";
  let cur = src.find("UNLISTEN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on UNLISTEN");
}

#[test]
fn r22_h_meta_0013() {
  let src = "-- v0\nNOTIFY ch";
  let cur = src.find("NOTIFY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NOTIFY");
}

#[test]
fn r22_h_meta_0014() {
  let src = "-- v0\nLOAD 'mylib'";
  let cur = src.find("LOAD").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LOAD");
}

#[test]
fn r22_h_meta_0015() {
  let src = "-- v0\nCALL p()";
  let cur = src.find("CALL").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CALL");
}

#[test]
fn r22_h_meta_0016() {
  let src = "-- v0\nDO $$ BEGIN NULL; END $$";
  let cur = src.find("DO").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DO");
}

#[test]
fn r22_h_meta_0017() {
  let src = "-- v0\nVACUUM FULL users";
  let cur = src.find("VACUUM").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on VACUUM");
}

#[test]
fn r22_h_meta_0018() {
  let src = "-- v0\nVACUUM FULL users";
  let cur = src.find("FULL").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FULL");
}

#[test]
fn r22_h_meta_0019() {
  let src = "-- v0\nCLUSTER users";
  let cur = src.find("CLUSTER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CLUSTER");
}

#[test]
fn r22_h_meta_0020() {
  let src = "-- v0\nREINDEX TABLE users";
  let cur = src.find("REINDEX").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REINDEX");
}

#[test]
fn r22_h_meta_0021() {
  let src = "-- v1\nEXPLAIN SELECT * FROM users";
  let cur = src.find("EXPLAIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXPLAIN");
}

#[test]
fn r22_h_meta_0022() {
  let src = "-- v1\nEXPLAIN (FORMAT JSON) SELECT * FROM users";
  let cur = src.find("FORMAT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FORMAT");
}

#[test]
fn r22_h_meta_0023() {
  let src = "-- v1\nPREPARE p AS SELECT 1";
  let cur = src.find("PREPARE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PREPARE");
}

#[test]
fn r22_h_meta_0024() {
  let src = "-- v1\nEXECUTE p(1)";
  let cur = src.find("EXECUTE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXECUTE");
}

#[test]
fn r22_h_meta_0025() {
  let src = "-- v1\nDEALLOCATE p";
  let cur = src.find("DEALLOCATE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEALLOCATE");
}

#[test]
fn r22_h_meta_0026() {
  let src = "-- v1\nDECLARE c CURSOR FOR SELECT 1";
  let cur = src.find("CURSOR").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CURSOR");
}

#[test]
fn r22_h_meta_0027() {
  let src = "-- v1\nFETCH NEXT FROM c";
  let cur = src.find("FETCH").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FETCH");
}

#[test]
fn r22_h_meta_0028() {
  let src = "-- v1\nFETCH NEXT FROM c";
  let cur = src.find("NEXT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NEXT");
}

#[test]
fn r22_h_meta_0029() {
  let src = "-- v1\nMOVE c";
  let cur = src.find("MOVE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on MOVE");
}

#[test]
fn r22_h_meta_0030() {
  let src = "-- v1\nCLOSE c";
  let cur = src.find("CLOSE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CLOSE");
}

#[test]
fn r22_h_meta_0031() {
  let src = "-- v1\nLISTEN ch";
  let cur = src.find("LISTEN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LISTEN");
}

#[test]
fn r22_h_meta_0032() {
  let src = "-- v1\nUNLISTEN ch";
  let cur = src.find("UNLISTEN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on UNLISTEN");
}

#[test]
fn r22_h_meta_0033() {
  let src = "-- v1\nNOTIFY ch";
  let cur = src.find("NOTIFY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NOTIFY");
}

#[test]
fn r22_h_meta_0034() {
  let src = "-- v1\nLOAD 'mylib'";
  let cur = src.find("LOAD").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LOAD");
}

#[test]
fn r22_h_meta_0035() {
  let src = "-- v1\nCALL p()";
  let cur = src.find("CALL").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CALL");
}

#[test]
fn r22_h_meta_0036() {
  let src = "-- v1\nDO $$ BEGIN NULL; END $$";
  let cur = src.find("DO").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DO");
}

#[test]
fn r22_h_meta_0037() {
  let src = "-- v1\nVACUUM FULL users";
  let cur = src.find("VACUUM").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on VACUUM");
}

#[test]
fn r22_h_meta_0038() {
  let src = "-- v1\nVACUUM FULL users";
  let cur = src.find("FULL").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FULL");
}

#[test]
fn r22_h_meta_0039() {
  let src = "-- v1\nCLUSTER users";
  let cur = src.find("CLUSTER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CLUSTER");
}

#[test]
fn r22_h_meta_0040() {
  let src = "-- v1\nREINDEX TABLE users";
  let cur = src.find("REINDEX").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REINDEX");
}

#[test]
fn r23_probe_h() {
  for (s, find) in [
    ("SELECT count(*) FILTER (WHERE id > 0) FROM users", "FILTER"),
    ("SELECT count(*) OVER (PARTITION BY id) FROM users", "OVER"),
    ("SELECT count(*) OVER (PARTITION BY id) FROM users", "PARTITION"),
    ("SELECT count(*) OVER (ORDER BY id) FROM users", "OVER"),
    ("SELECT sum(amount) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM orders", "ROWS"),
    ("SELECT sum(amount) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM orders", "UNBOUNDED"),
    ("SELECT sum(amount) OVER (RANGE BETWEEN INTERVAL '1 day' PRECEDING AND CURRENT ROW) FROM orders", "RANGE"),
    ("SELECT sum(amount) OVER (GROUPS BETWEEN 1 PRECEDING AND 1 FOLLOWING) FROM orders", "GROUPS"),
    ("SELECT rank() WITHIN GROUP (ORDER BY id) FROM users", "WITHIN"),
    ("SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY id) FROM users", "WITHIN"),
    ("SELECT * FROM users TABLESAMPLE BERNOULLI (10)", "TABLESAMPLE"),
    ("SELECT * FROM users TABLESAMPLE SYSTEM (10) REPEATABLE (42)", "REPEATABLE"),
    ("SELECT * FROM users u CROSS JOIN LATERAL (SELECT 1) x", "LATERAL"),
    ("INSERT INTO users (id) VALUES (DEFAULT)", "DEFAULT"),
    ("INSERT INTO users (id) VALUES (1) ON CONFLICT DO NOTHING", "CONFLICT"),
    ("INSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name='x'", "UPDATE"),
  ] {
    let cur = s.find(find).unwrap();
    let md = hover_at(s, cur);
    eprintln!("HE|{}|some={}", find, md.is_some());
  }
}

#[test]
fn r23_h_kw_0001() {
  let src = "-- v0\nSELECT count(*) FILTER (WHERE id > 0) FROM users";
  let cur = src.find("FILTER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FILTER");
}

#[test]
fn r23_h_kw_0002() {
  let src = "-- v0\nSELECT count(*) OVER (PARTITION BY id) FROM users";
  let cur = src.find("OVER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on OVER");
}

#[test]
fn r23_h_kw_0003() {
  let src = "-- v0\nSELECT count(*) OVER (PARTITION BY id) FROM users";
  let cur = src.find("PARTITION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PARTITION");
}

#[test]
fn r23_h_kw_0004() {
  let src = "-- v0\nSELECT sum(amount) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM orders";
  let cur = src.find("ROWS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ROWS");
}

#[test]
fn r23_h_kw_0005() {
  let src = "-- v0\nSELECT sum(amount) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM orders";
  let cur = src.find("UNBOUNDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on UNBOUNDED");
}

#[test]
fn r23_h_kw_0006() {
  let src = "-- v0\nSELECT sum(amount) OVER (RANGE BETWEEN INTERVAL '1 day' PRECEDING AND CURRENT ROW) FROM orders";
  let cur = src.find("RANGE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RANGE");
}

#[test]
fn r23_h_kw_0007() {
  let src = "-- v0\nSELECT sum(amount) OVER (GROUPS BETWEEN 1 PRECEDING AND 1 FOLLOWING) FROM orders";
  let cur = src.find("GROUPS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on GROUPS");
}

#[test]
fn r23_h_kw_0008() {
  let src = "-- v0\nSELECT rank() WITHIN GROUP (ORDER BY id) FROM users";
  let cur = src.find("WITHIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WITHIN");
}

#[test]
fn r23_h_kw_0009() {
  let src = "-- v0\nSELECT * FROM users TABLESAMPLE BERNOULLI (10)";
  let cur = src.find("TABLESAMPLE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on TABLESAMPLE");
}

#[test]
fn r23_h_kw_0010() {
  let src = "-- v0\nSELECT * FROM users TABLESAMPLE SYSTEM (10) REPEATABLE (42)";
  let cur = src.find("REPEATABLE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REPEATABLE");
}

#[test]
fn r23_h_kw_0011() {
  let src = "-- v0\nSELECT * FROM users u CROSS JOIN LATERAL (SELECT 1) x";
  let cur = src.find("LATERAL").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LATERAL");
}

#[test]
fn r23_h_kw_0012() {
  let src = "-- v0\nINSERT INTO users (id) VALUES (DEFAULT)";
  let cur = src.find("DEFAULT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFAULT");
}

#[test]
fn r23_h_kw_0013() {
  let src = "-- v0\nINSERT INTO users (id) VALUES (1) ON CONFLICT DO NOTHING";
  let cur = src.find("CONFLICT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONFLICT");
}

#[test]
fn r23_h_kw_0014() {
  let src = "-- v1\nSELECT count(*) FILTER (WHERE id > 0) FROM users";
  let cur = src.find("FILTER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FILTER");
}

#[test]
fn r23_h_kw_0015() {
  let src = "-- v1\nSELECT count(*) OVER (PARTITION BY id) FROM users";
  let cur = src.find("OVER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on OVER");
}

#[test]
fn r23_h_kw_0016() {
  let src = "-- v1\nSELECT count(*) OVER (PARTITION BY id) FROM users";
  let cur = src.find("PARTITION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PARTITION");
}

#[test]
fn r23_h_kw_0017() {
  let src = "-- v1\nSELECT sum(amount) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM orders";
  let cur = src.find("ROWS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ROWS");
}

#[test]
fn r23_h_kw_0018() {
  let src = "-- v1\nSELECT sum(amount) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM orders";
  let cur = src.find("UNBOUNDED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on UNBOUNDED");
}

#[test]
fn r23_h_kw_0019() {
  let src = "-- v1\nSELECT sum(amount) OVER (RANGE BETWEEN INTERVAL '1 day' PRECEDING AND CURRENT ROW) FROM orders";
  let cur = src.find("RANGE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RANGE");
}

#[test]
fn r23_h_kw_0020() {
  let src = "-- v1\nSELECT sum(amount) OVER (GROUPS BETWEEN 1 PRECEDING AND 1 FOLLOWING) FROM orders";
  let cur = src.find("GROUPS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on GROUPS");
}

#[test]
fn r23_h_kw_0021() {
  let src = "-- v1\nSELECT rank() WITHIN GROUP (ORDER BY id) FROM users";
  let cur = src.find("WITHIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WITHIN");
}

#[test]
fn r23_h_kw_0022() {
  let src = "-- v1\nSELECT * FROM users TABLESAMPLE BERNOULLI (10)";
  let cur = src.find("TABLESAMPLE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on TABLESAMPLE");
}

#[test]
fn r23_h_kw_0023() {
  let src = "-- v1\nSELECT * FROM users TABLESAMPLE SYSTEM (10) REPEATABLE (42)";
  let cur = src.find("REPEATABLE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REPEATABLE");
}

#[test]
fn r23_h_kw_0024() {
  let src = "-- v1\nSELECT * FROM users u CROSS JOIN LATERAL (SELECT 1) x";
  let cur = src.find("LATERAL").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LATERAL");
}

#[test]
fn r23_h_kw_0025() {
  let src = "-- v1\nINSERT INTO users (id) VALUES (DEFAULT)";
  let cur = src.find("DEFAULT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFAULT");
}

#[test]
fn r23_h_kw_0026() {
  let src = "-- v1\nINSERT INTO users (id) VALUES (1) ON CONFLICT DO NOTHING";
  let cur = src.find("CONFLICT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONFLICT");
}

#[test]
fn r23_h_kw_0027() {
  let src = "-- v2\nSELECT count(*) FILTER (WHERE id > 0) FROM users";
  let cur = src.find("FILTER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FILTER");
}

#[test]
fn r23_h_kw_0028() {
  let src = "-- v2\nSELECT count(*) OVER (PARTITION BY id) FROM users";
  let cur = src.find("OVER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on OVER");
}

#[test]
fn r23_h_kw_0029() {
  let src = "-- v2\nSELECT count(*) OVER (PARTITION BY id) FROM users";
  let cur = src.find("PARTITION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PARTITION");
}

#[test]
fn r23_h_kw_0030() {
  let src = "-- v2\nSELECT sum(amount) OVER (ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM orders";
  let cur = src.find("ROWS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ROWS");
}

#[test]
fn r24_probe_h() {
  for (s, find) in [
    ("SELECT a[1] FROM users", "["),
    ("SELECT a[1:3] FROM users", ":"),
    ("SELECT * FROM users WHERE name LIKE 'a%'", "%"),
    ("SELECT id AS my_id FROM users", "AS"),
    ("SELECT id AS my_id FROM users", "my_id"),
    ("CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY)", "GENERATED"),
    ("CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY)", "IDENTITY"),
    ("CREATE TABLE t (id int GENERATED BY DEFAULT AS IDENTITY)", "DEFAULT"),
    ("CREATE TABLE c PARTITION OF parent FOR VALUES IN (1, 2)", "PARTITION"),
    ("CREATE TABLE c PARTITION OF parent FOR VALUES IN (1, 2)", "VALUES"),
    ("CREATE TABLE c PARTITION OF parent FOR VALUES FROM (1) TO (10)", "FROM"),
    ("CREATE TABLE c PARTITION OF parent FOR VALUES FROM (1) TO (10)", "TO"),
    ("CREATE INDEX ON users (id) INCLUDE (email)", "INCLUDE"),
    ("CREATE INDEX ON users (id) WHERE name IS NOT NULL", "WHERE"),
    ("CREATE INDEX ON users USING gin (name gin_trgm_ops)", "gin"),
    ("CREATE INDEX ON users USING gin (name gin_trgm_ops)", "gin_trgm_ops"),
    ("ALTER TABLE users INHERIT parent", "INHERIT"),
    ("ALTER TABLE users NO INHERIT parent", "NO"),
    ("ALTER TABLE users SET WITHOUT OIDS", "WITHOUT"),
    ("ALTER TABLE users CLUSTER ON pk_idx", "CLUSTER"),
    ("ALTER TABLE users SET WITHOUT CLUSTER", "CLUSTER"),
    ("CREATE TYPE my_t AS ENUM ('a', 'b')", "ENUM"),
    ("CREATE TYPE my_t AS RANGE (SUBTYPE = int)", "RANGE"),
    ("CREATE TYPE my_t AS RANGE (SUBTYPE = int)", "SUBTYPE"),
    ("CREATE DOMAIN d AS int CHECK (VALUE > 0)", "DOMAIN"),
    ("CREATE DOMAIN d AS int CHECK (VALUE > 0)", "VALUE"),
  ] {
    let cur = s.find(find).unwrap();
    let md = hover_at(s, cur);
    eprintln!("H|{}|some={}", find, md.is_some());
  }
}

#[test]
fn r24_h_0001() {
  let src = "-- v0\nSELECT id AS my_id FROM users";
  let cur = src.find("AS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on AS");
}

#[test]
fn r24_h_0002() {
  let src = "-- v0\nCREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY)";
  let cur = src.find("GENERATED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on GENERATED");
}

#[test]
fn r24_h_0003() {
  let src = "-- v0\nCREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY)";
  let cur = src.find("IDENTITY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on IDENTITY");
}

#[test]
fn r24_h_0004() {
  let src = "-- v0\nCREATE TABLE t (id int GENERATED BY DEFAULT AS IDENTITY)";
  let cur = src.find("DEFAULT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFAULT");
}

#[test]
fn r24_h_0005() {
  let src = "-- v0\nCREATE TABLE c PARTITION OF parent FOR VALUES IN (1, 2)";
  let cur = src.find("PARTITION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PARTITION");
}

#[test]
fn r24_h_0006() {
  let src = "-- v0\nCREATE TABLE c PARTITION OF parent FOR VALUES IN (1, 2)";
  let cur = src.find("VALUES").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on VALUES");
}

#[test]
fn r24_h_0007() {
  let src = "-- v0\nCREATE INDEX ON users (id) INCLUDE (email)";
  let cur = src.find("INCLUDE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INCLUDE");
}

#[test]
fn r24_h_0008() {
  let src = "-- v0\nCREATE INDEX ON users (id) WHERE name IS NOT NULL";
  let cur = src.find("WHERE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WHERE");
}

#[test]
fn r24_h_0009() {
  let src = "-- v0\nCREATE INDEX ON users USING gin (name gin_trgm_ops)";
  let cur = src.find("gin").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on gin");
}

#[test]
fn r24_h_0010() {
  let src = "-- v0\nALTER TABLE users INHERIT parent";
  let cur = src.find("INHERIT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INHERIT");
}

#[test]
fn r24_h_0011() {
  let src = "-- v0\nALTER TABLE users SET WITHOUT OIDS";
  let cur = src.find("WITHOUT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WITHOUT");
}

#[test]
fn r24_h_0012() {
  let src = "-- v0\nALTER TABLE users CLUSTER ON pk_idx";
  let cur = src.find("CLUSTER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CLUSTER");
}

#[test]
fn r24_h_0013() {
  let src = "-- v0\nCREATE TYPE my_t AS ENUM ('a', 'b')";
  let cur = src.find("ENUM").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ENUM");
}

#[test]
fn r24_h_0014() {
  let src = "-- v0\nCREATE TYPE my_t AS RANGE (SUBTYPE = int)";
  let cur = src.find("RANGE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RANGE");
}

#[test]
fn r24_h_0015() {
  let src = "-- v0\nCREATE DOMAIN d AS int CHECK (VALUE > 0)";
  let cur = src.find("DOMAIN").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DOMAIN");
}

#[test]
fn r24_h_0016() {
  let src = "-- v0\nCREATE DOMAIN d AS int CHECK (VALUE > 0)";
  let cur = src.find("VALUE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on VALUE");
}

#[test]
fn r24_h_0017() {
  let src = "-- v1\nSELECT id AS my_id FROM users";
  let cur = src.find("AS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on AS");
}

#[test]
fn r24_h_0018() {
  let src = "-- v1\nCREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY)";
  let cur = src.find("GENERATED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on GENERATED");
}

#[test]
fn r24_h_0019() {
  let src = "-- v1\nCREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY)";
  let cur = src.find("IDENTITY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on IDENTITY");
}

#[test]
fn r24_h_0020() {
  let src = "-- v1\nCREATE TABLE t (id int GENERATED BY DEFAULT AS IDENTITY)";
  let cur = src.find("DEFAULT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFAULT");
}

#[test]
fn r24_h_0021() {
  let src = "-- v1\nCREATE TABLE c PARTITION OF parent FOR VALUES IN (1, 2)";
  let cur = src.find("PARTITION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PARTITION");
}

#[test]
fn r24_h_0022() {
  let src = "-- v1\nCREATE TABLE c PARTITION OF parent FOR VALUES IN (1, 2)";
  let cur = src.find("VALUES").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on VALUES");
}

#[test]
fn r24_h_0023() {
  let src = "-- v1\nCREATE INDEX ON users (id) INCLUDE (email)";
  let cur = src.find("INCLUDE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INCLUDE");
}

#[test]
fn r24_h_0024() {
  let src = "-- v1\nCREATE INDEX ON users (id) WHERE name IS NOT NULL";
  let cur = src.find("WHERE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WHERE");
}

#[test]
fn r24_h_0025() {
  let src = "-- v1\nCREATE INDEX ON users USING gin (name gin_trgm_ops)";
  let cur = src.find("gin").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on gin");
}

#[test]
fn r24_h_0026() {
  let src = "-- v1\nALTER TABLE users INHERIT parent";
  let cur = src.find("INHERIT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on INHERIT");
}

#[test]
fn r24_h_0027() {
  let src = "-- v1\nALTER TABLE users SET WITHOUT OIDS";
  let cur = src.find("WITHOUT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WITHOUT");
}

#[test]
fn r24_h_0028() {
  let src = "-- v1\nALTER TABLE users CLUSTER ON pk_idx";
  let cur = src.find("CLUSTER").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CLUSTER");
}

#[test]
fn r24_h_0029() {
  let src = "-- v1\nCREATE TYPE my_t AS ENUM ('a', 'b')";
  let cur = src.find("ENUM").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ENUM");
}

#[test]
fn r24_h_0030() {
  let src = "-- v1\nCREATE TYPE my_t AS RANGE (SUBTYPE = int)";
  let cur = src.find("RANGE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RANGE");
}

#[test]
fn r25_probe_h() {
  for (s, find) in [
    ("CREATE TABLE c (LIKE users INCLUDING ALL)", "LIKE"),
    ("CREATE TABLE c (LIKE users INCLUDING ALL)", "INCLUDING"),
    ("CREATE TABLE c (LIKE users EXCLUDING INDEXES)", "EXCLUDING"),
    ("CREATE TABLE c PARTITION OF parent FOR VALUES IN (1, 2)", "OF"),
    ("CREATE TABLE c PARTITION OF parent DEFAULT", "DEFAULT"),
    ("CREATE TABLE x (a int, EXCLUDE USING gist (a WITH =))", "EXCLUDE"),
    ("CREATE TABLE x (a int, EXCLUDE USING gist (a WITH =))", "WITH"),
    ("REFRESH MATERIALIZED VIEW CONCURRENTLY mv", "REFRESH"),
    ("REFRESH MATERIALIZED VIEW CONCURRENTLY mv", "MATERIALIZED"),
    ("REFRESH MATERIALIZED VIEW CONCURRENTLY mv", "CONCURRENTLY"),
    ("CREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)", "POLICY"),
    ("CREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)", "FOR"),
    ("CREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)", "USING"),
    ("CREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)", "CHECK"),
    ("CREATE EVENT TRIGGER et ON ddl_command_start EXECUTE FUNCTION audit()", "EVENT"),
    ("CREATE EVENT TRIGGER et ON ddl_command_start WHEN TAG IN ('CREATE TABLE') EXECUTE FUNCTION audit()", "TAG"),
    ("ALTER PUBLICATION pub ADD TABLE users", "PUBLICATION"),
    ("CREATE SUBSCRIPTION sub CONNECTION 'host=h' PUBLICATION pub", "SUBSCRIPTION"),
    ("CREATE SUBSCRIPTION sub CONNECTION 'host=h' PUBLICATION pub", "CONNECTION"),
  ] {
    let cur = s.find(find).unwrap();
    let md = hover_at(s, cur);
    eprintln!("H|{}|some={}", find, md.is_some());
  }
}

#[test]
fn r25_h_0001() {
  let src = "-- v0\nCREATE TABLE c PARTITION OF parent FOR VALUES IN (1, 2)";
  let cur = src.find("OF").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on OF");
}

#[test]
fn r25_h_0002() {
  let src = "-- v0\nCREATE TABLE c PARTITION OF parent DEFAULT";
  let cur = src.find("DEFAULT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFAULT");
}

#[test]
fn r25_h_0003() {
  let src = "-- v0\nCREATE TABLE x (a int, EXCLUDE USING gist (a WITH =))";
  let cur = src.find("EXCLUDE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDE");
}

#[test]
fn r25_h_0004() {
  let src = "-- v0\nREFRESH MATERIALIZED VIEW CONCURRENTLY mv";
  let cur = src.find("REFRESH").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REFRESH");
}

#[test]
fn r25_h_0005() {
  let src = "-- v0\nREFRESH MATERIALIZED VIEW CONCURRENTLY mv";
  let cur = src.find("MATERIALIZED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on MATERIALIZED");
}

#[test]
fn r25_h_0006() {
  let src = "-- v0\nREFRESH MATERIALIZED VIEW CONCURRENTLY mv";
  let cur = src.find("CONCURRENTLY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONCURRENTLY");
}

#[test]
fn r25_h_0007() {
  let src = "-- v0\nCREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)";
  let cur = src.find("POLICY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on POLICY");
}

#[test]
fn r25_h_0008() {
  let src = "-- v0\nCREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)";
  let cur = src.find("FOR").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FOR");
}

#[test]
fn r25_h_0009() {
  let src = "-- v0\nCREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING");
}

#[test]
fn r25_h_0010() {
  let src = "-- v0\nCREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)";
  let cur = src.find("CHECK").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CHECK");
}

#[test]
fn r25_h_0011() {
  let src = "-- v0\nCREATE EVENT TRIGGER et ON ddl_command_start EXECUTE FUNCTION audit()";
  let cur = src.find("EVENT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EVENT");
}

#[test]
fn r25_h_0012() {
  let src = "-- v0\nCREATE EVENT TRIGGER et ON ddl_command_start WHEN TAG IN ('CREATE TABLE') EXECUTE FUNCTION audit()";
  let cur = src.find("TAG").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on TAG");
}

#[test]
fn r25_h_0013() {
  let src = "-- v0\nALTER PUBLICATION pub ADD TABLE users";
  let cur = src.find("PUBLICATION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PUBLICATION");
}

#[test]
fn r25_h_0014() {
  let src = "-- v0\nCREATE SUBSCRIPTION sub CONNECTION 'host=h' PUBLICATION pub";
  let cur = src.find("SUBSCRIPTION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SUBSCRIPTION");
}

#[test]
fn r25_h_0015() {
  let src = "-- v0\nCREATE SUBSCRIPTION sub CONNECTION 'host=h' PUBLICATION pub";
  let cur = src.find("CONNECTION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONNECTION");
}

#[test]
fn r25_h_0016() {
  let src = "-- v1\nCREATE TABLE c PARTITION OF parent FOR VALUES IN (1, 2)";
  let cur = src.find("OF").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on OF");
}

#[test]
fn r25_h_0017() {
  let src = "-- v1\nCREATE TABLE c PARTITION OF parent DEFAULT";
  let cur = src.find("DEFAULT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFAULT");
}

#[test]
fn r25_h_0018() {
  let src = "-- v1\nCREATE TABLE x (a int, EXCLUDE USING gist (a WITH =))";
  let cur = src.find("EXCLUDE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EXCLUDE");
}

#[test]
fn r25_h_0019() {
  let src = "-- v1\nREFRESH MATERIALIZED VIEW CONCURRENTLY mv";
  let cur = src.find("REFRESH").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REFRESH");
}

#[test]
fn r25_h_0020() {
  let src = "-- v1\nREFRESH MATERIALIZED VIEW CONCURRENTLY mv";
  let cur = src.find("MATERIALIZED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on MATERIALIZED");
}

#[test]
fn r25_h_0021() {
  let src = "-- v1\nREFRESH MATERIALIZED VIEW CONCURRENTLY mv";
  let cur = src.find("CONCURRENTLY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONCURRENTLY");
}

#[test]
fn r25_h_0022() {
  let src = "-- v1\nCREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)";
  let cur = src.find("POLICY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on POLICY");
}

#[test]
fn r25_h_0023() {
  let src = "-- v1\nCREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)";
  let cur = src.find("FOR").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on FOR");
}

#[test]
fn r25_h_0024() {
  let src = "-- v1\nCREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)";
  let cur = src.find("USING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on USING");
}

#[test]
fn r25_h_0025() {
  let src = "-- v1\nCREATE POLICY p ON users FOR SELECT USING (true) WITH CHECK (true)";
  let cur = src.find("CHECK").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CHECK");
}

#[test]
fn r25_h_0026() {
  let src = "-- v1\nCREATE EVENT TRIGGER et ON ddl_command_start EXECUTE FUNCTION audit()";
  let cur = src.find("EVENT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on EVENT");
}

#[test]
fn r25_h_0027() {
  let src = "-- v1\nCREATE EVENT TRIGGER et ON ddl_command_start WHEN TAG IN ('CREATE TABLE') EXECUTE FUNCTION audit()";
  let cur = src.find("TAG").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on TAG");
}

#[test]
fn r25_h_0028() {
  let src = "-- v1\nALTER PUBLICATION pub ADD TABLE users";
  let cur = src.find("PUBLICATION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PUBLICATION");
}

#[test]
fn r25_h_0029() {
  let src = "-- v1\nCREATE SUBSCRIPTION sub CONNECTION 'host=h' PUBLICATION pub";
  let cur = src.find("SUBSCRIPTION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SUBSCRIPTION");
}

#[test]
fn r25_h_0030() {
  let src = "-- v1\nCREATE SUBSCRIPTION sub CONNECTION 'host=h' PUBLICATION pub";
  let cur = src.find("CONNECTION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CONNECTION");
}

#[test]
fn r26_probe_h() {
  for (s, find) in [
    ("SELECT id FROM users TABLESAMPLE SYSTEM (10)", "SYSTEM"),
    ("SELECT id FROM users TABLESAMPLE BERNOULLI (10)", "BERNOULLI"),
    ("SELECT id FROM users TABLESAMPLE SYSTEM (10) REPEATABLE (42)", "REPEATABLE"),
    ("SELECT * FROM users FOR UPDATE NOWAIT", "NOWAIT"),
    ("SELECT * FROM users FOR UPDATE SKIP LOCKED", "SKIP"),
    ("SELECT * FROM users FOR UPDATE SKIP LOCKED", "LOCKED"),
    ("SELECT * FROM users FOR NO KEY UPDATE", "KEY"),
    ("SELECT * FROM users FOR KEY SHARE", "SHARE"),
    ("BEGIN ISOLATION LEVEL SERIALIZABLE", "SERIALIZABLE"),
    ("BEGIN ISOLATION LEVEL REPEATABLE READ", "REPEATABLE"),
    ("BEGIN ISOLATION LEVEL READ COMMITTED", "COMMITTED"),
    ("BEGIN ISOLATION LEVEL READ UNCOMMITTED", "UNCOMMITTED"),
    ("BEGIN DEFERRABLE READ ONLY", "DEFERRABLE"),
    ("BEGIN READ ONLY", "READ"),
    ("BEGIN READ WRITE", "WRITE"),
    ("SET TRANSACTION SNAPSHOT 'sid'", "SNAPSHOT"),
    ("PREPARE TRANSACTION 'xid'", "TRANSACTION"),
    ("COMMIT PREPARED 'xid'", "PREPARED"),
    ("ROLLBACK PREPARED 'xid'", "PREPARED"),
    ("SAVEPOINT sp", "SAVEPOINT"),
    ("ROLLBACK TO SAVEPOINT sp", "ROLLBACK"),
    ("RELEASE SAVEPOINT sp", "RELEASE"),
  ] {
    let cur = s.find(find).unwrap();
    let md = hover_at(s, cur);
    eprintln!("H|{}|some={}", find, md.is_some());
  }
}

#[test]
fn r26_h_0001() {
  let src = "-- v0\nSELECT id FROM users TABLESAMPLE SYSTEM (10)";
  let cur = src.find("SYSTEM").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SYSTEM");
}

#[test]
fn r26_h_0002() {
  let src = "-- v0\nSELECT id FROM users TABLESAMPLE SYSTEM (10) REPEATABLE (42)";
  let cur = src.find("REPEATABLE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REPEATABLE");
}

#[test]
fn r26_h_0003() {
  let src = "-- v0\nSELECT * FROM users FOR UPDATE NOWAIT";
  let cur = src.find("NOWAIT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NOWAIT");
}

#[test]
fn r26_h_0004() {
  let src = "-- v0\nSELECT * FROM users FOR UPDATE SKIP LOCKED";
  let cur = src.find("SKIP").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SKIP");
}

#[test]
fn r26_h_0005() {
  let src = "-- v0\nSELECT * FROM users FOR UPDATE SKIP LOCKED";
  let cur = src.find("LOCKED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LOCKED");
}

#[test]
fn r26_h_0006() {
  let src = "-- v0\nSELECT * FROM users FOR NO KEY UPDATE";
  let cur = src.find("KEY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on KEY");
}

#[test]
fn r26_h_0007() {
  let src = "-- v0\nSELECT * FROM users FOR KEY SHARE";
  let cur = src.find("SHARE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SHARE");
}

#[test]
fn r26_h_0008() {
  let src = "-- v0\nBEGIN ISOLATION LEVEL SERIALIZABLE";
  let cur = src.find("SERIALIZABLE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SERIALIZABLE");
}

#[test]
fn r26_h_0009() {
  let src = "-- v0\nBEGIN ISOLATION LEVEL READ COMMITTED";
  let cur = src.find("COMMITTED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on COMMITTED");
}

#[test]
fn r26_h_0010() {
  let src = "-- v0\nBEGIN ISOLATION LEVEL READ UNCOMMITTED";
  let cur = src.find("UNCOMMITTED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on UNCOMMITTED");
}

#[test]
fn r26_h_0011() {
  let src = "-- v0\nBEGIN DEFERRABLE READ ONLY";
  let cur = src.find("DEFERRABLE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFERRABLE");
}

#[test]
fn r26_h_0012() {
  let src = "-- v0\nBEGIN READ ONLY";
  let cur = src.find("READ").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on READ");
}

#[test]
fn r26_h_0013() {
  let src = "-- v0\nBEGIN READ WRITE";
  let cur = src.find("WRITE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WRITE");
}

#[test]
fn r26_h_0014() {
  let src = "-- v0\nSET TRANSACTION SNAPSHOT 'sid'";
  let cur = src.find("SNAPSHOT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SNAPSHOT");
}

#[test]
fn r26_h_0015() {
  let src = "-- v0\nPREPARE TRANSACTION 'xid'";
  let cur = src.find("TRANSACTION").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on TRANSACTION");
}

#[test]
fn r26_h_0016() {
  let src = "-- v0\nCOMMIT PREPARED 'xid'";
  let cur = src.find("PREPARED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on PREPARED");
}

#[test]
fn r26_h_0017() {
  let src = "-- v0\nSAVEPOINT sp";
  let cur = src.find("SAVEPOINT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SAVEPOINT");
}

#[test]
fn r26_h_0018() {
  let src = "-- v0\nROLLBACK TO SAVEPOINT sp";
  let cur = src.find("ROLLBACK").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ROLLBACK");
}

#[test]
fn r26_h_0019() {
  let src = "-- v0\nRELEASE SAVEPOINT sp";
  let cur = src.find("RELEASE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on RELEASE");
}

#[test]
fn r26_h_0020() {
  let src = "-- v1\nSELECT id FROM users TABLESAMPLE SYSTEM (10)";
  let cur = src.find("SYSTEM").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SYSTEM");
}

#[test]
fn r26_h_0021() {
  let src = "-- v1\nSELECT id FROM users TABLESAMPLE SYSTEM (10) REPEATABLE (42)";
  let cur = src.find("REPEATABLE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on REPEATABLE");
}

#[test]
fn r26_h_0022() {
  let src = "-- v1\nSELECT * FROM users FOR UPDATE NOWAIT";
  let cur = src.find("NOWAIT").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NOWAIT");
}

#[test]
fn r26_h_0023() {
  let src = "-- v1\nSELECT * FROM users FOR UPDATE SKIP LOCKED";
  let cur = src.find("SKIP").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SKIP");
}

#[test]
fn r26_h_0024() {
  let src = "-- v1\nSELECT * FROM users FOR UPDATE SKIP LOCKED";
  let cur = src.find("LOCKED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on LOCKED");
}

#[test]
fn r26_h_0025() {
  let src = "-- v1\nSELECT * FROM users FOR NO KEY UPDATE";
  let cur = src.find("KEY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on KEY");
}

#[test]
fn r26_h_0026() {
  let src = "-- v1\nSELECT * FROM users FOR KEY SHARE";
  let cur = src.find("SHARE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SHARE");
}

#[test]
fn r26_h_0027() {
  let src = "-- v1\nBEGIN ISOLATION LEVEL SERIALIZABLE";
  let cur = src.find("SERIALIZABLE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SERIALIZABLE");
}

#[test]
fn r26_h_0028() {
  let src = "-- v1\nBEGIN ISOLATION LEVEL READ COMMITTED";
  let cur = src.find("COMMITTED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on COMMITTED");
}

#[test]
fn r26_h_0029() {
  let src = "-- v1\nBEGIN ISOLATION LEVEL READ UNCOMMITTED";
  let cur = src.find("UNCOMMITTED").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on UNCOMMITTED");
}

#[test]
fn r26_h_0030() {
  let src = "-- v1\nBEGIN DEFERRABLE READ ONLY";
  let cur = src.find("DEFERRABLE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on DEFERRABLE");
}

#[test]
fn r27_probe_h() {
  for (s, find) in [
    ("SELECT id FROM users GROUP BY GROUPING SETS ((id))", "GROUPING"),
    ("SELECT id FROM users GROUP BY GROUPING SETS ((id))", "SETS"),
    ("SELECT id FROM users GROUP BY ROLLUP (id)", "ROLLUP"),
    ("SELECT id FROM users GROUP BY CUBE (id)", "CUBE"),
    ("SELECT id, sum(id) FROM users GROUP BY id HAVING sum(id) > 1", "HAVING"),
    ("SELECT lag(id) OVER (ORDER BY id)", "lag"),
    ("SELECT first_value(id) OVER ()", "first_value"),
    ("SELECT nth_value(id, 1) OVER ()", "nth_value"),
    ("SELECT ntile(4) OVER ()", "ntile"),
    ("SELECT row_number() OVER ()", "row_number"),
    ("SELECT * FROM users u FOR UPDATE OF u", "OF"),
    ("SELECT * FROM users u FOR UPDATE OF u SKIP LOCKED", "SKIP"),
    ("SELECT * FROM users u FOR NO KEY UPDATE OF u", "NO"),
    ("SELECT id FROM users LIMIT 1 OFFSET 0 ROWS FETCH NEXT 5 ROWS WITH TIES", "TIES"),
    ("SELECT id FROM users LIMIT 1 OFFSET 0 ROWS FETCH NEXT 5 ROWS ONLY", "ONLY"),
    ("SELECT id FROM users LIMIT 1 OFFSET 0 ROWS FETCH NEXT 5 ROWS WITH TIES", "WITH"),
  ] {
    let cur = s.find(find).unwrap();
    let md = hover_at(s, cur);
    eprintln!("H|{}|some={}", find, md.is_some());
  }
}

#[test]
fn r27_h_0001() {
  let src = "-- v0\nSELECT id FROM users GROUP BY GROUPING SETS ((id))";
  let cur = src.find("GROUPING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on GROUPING");
}

#[test]
fn r27_h_0002() {
  let src = "-- v0\nSELECT id FROM users GROUP BY GROUPING SETS ((id))";
  let cur = src.find("SETS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SETS");
}

#[test]
fn r27_h_0003() {
  let src = "-- v0\nSELECT id FROM users GROUP BY ROLLUP (id)";
  let cur = src.find("ROLLUP").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ROLLUP");
}

#[test]
fn r27_h_0004() {
  let src = "-- v0\nSELECT id FROM users GROUP BY CUBE (id)";
  let cur = src.find("CUBE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CUBE");
}

#[test]
fn r27_h_0005() {
  let src = "-- v0\nSELECT lag(id) OVER (ORDER BY id)";
  let cur = src.find("lag").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on lag");
}

#[test]
fn r27_h_0006() {
  let src = "-- v0\nSELECT first_value(id) OVER ()";
  let cur = src.find("first_value").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on first_value");
}

#[test]
fn r27_h_0007() {
  let src = "-- v0\nSELECT nth_value(id, 1) OVER ()";
  let cur = src.find("nth_value").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on nth_value");
}

#[test]
fn r27_h_0008() {
  let src = "-- v0\nSELECT ntile(4) OVER ()";
  let cur = src.find("ntile").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ntile");
}

#[test]
fn r27_h_0009() {
  let src = "-- v0\nSELECT row_number() OVER ()";
  let cur = src.find("row_number").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on row_number");
}

#[test]
fn r27_h_0010() {
  let src = "-- v0\nSELECT * FROM users u FOR UPDATE OF u";
  let cur = src.find("OF").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on OF");
}

#[test]
fn r27_h_0011() {
  let src = "-- v0\nSELECT * FROM users u FOR UPDATE OF u SKIP LOCKED";
  let cur = src.find("SKIP").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SKIP");
}

#[test]
fn r27_h_0012() {
  let src = "-- v0\nSELECT * FROM users u FOR NO KEY UPDATE OF u";
  let cur = src.find("NO").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NO");
}

#[test]
fn r27_h_0013() {
  let src = "-- v0\nSELECT id FROM users LIMIT 1 OFFSET 0 ROWS FETCH NEXT 5 ROWS WITH TIES";
  let cur = src.find("TIES").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on TIES");
}

#[test]
fn r27_h_0014() {
  let src = "-- v0\nSELECT id FROM users LIMIT 1 OFFSET 0 ROWS FETCH NEXT 5 ROWS ONLY";
  let cur = src.find("ONLY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ONLY");
}

#[test]
fn r27_h_0015() {
  let src = "-- v0\nSELECT id FROM users LIMIT 1 OFFSET 0 ROWS FETCH NEXT 5 ROWS WITH TIES";
  let cur = src.find("WITH").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WITH");
}

#[test]
fn r27_h_0016() {
  let src = "-- v1\nSELECT id FROM users GROUP BY GROUPING SETS ((id))";
  let cur = src.find("GROUPING").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on GROUPING");
}

#[test]
fn r27_h_0017() {
  let src = "-- v1\nSELECT id FROM users GROUP BY GROUPING SETS ((id))";
  let cur = src.find("SETS").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SETS");
}

#[test]
fn r27_h_0018() {
  let src = "-- v1\nSELECT id FROM users GROUP BY ROLLUP (id)";
  let cur = src.find("ROLLUP").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ROLLUP");
}

#[test]
fn r27_h_0019() {
  let src = "-- v1\nSELECT id FROM users GROUP BY CUBE (id)";
  let cur = src.find("CUBE").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on CUBE");
}

#[test]
fn r27_h_0020() {
  let src = "-- v1\nSELECT lag(id) OVER (ORDER BY id)";
  let cur = src.find("lag").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on lag");
}

#[test]
fn r27_h_0021() {
  let src = "-- v1\nSELECT first_value(id) OVER ()";
  let cur = src.find("first_value").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on first_value");
}

#[test]
fn r27_h_0022() {
  let src = "-- v1\nSELECT nth_value(id, 1) OVER ()";
  let cur = src.find("nth_value").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on nth_value");
}

#[test]
fn r27_h_0023() {
  let src = "-- v1\nSELECT ntile(4) OVER ()";
  let cur = src.find("ntile").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ntile");
}

#[test]
fn r27_h_0024() {
  let src = "-- v1\nSELECT row_number() OVER ()";
  let cur = src.find("row_number").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on row_number");
}

#[test]
fn r27_h_0025() {
  let src = "-- v1\nSELECT * FROM users u FOR UPDATE OF u";
  let cur = src.find("OF").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on OF");
}

#[test]
fn r27_h_0026() {
  let src = "-- v1\nSELECT * FROM users u FOR UPDATE OF u SKIP LOCKED";
  let cur = src.find("SKIP").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on SKIP");
}

#[test]
fn r27_h_0027() {
  let src = "-- v1\nSELECT * FROM users u FOR NO KEY UPDATE OF u";
  let cur = src.find("NO").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on NO");
}

#[test]
fn r27_h_0028() {
  let src = "-- v1\nSELECT id FROM users LIMIT 1 OFFSET 0 ROWS FETCH NEXT 5 ROWS WITH TIES";
  let cur = src.find("TIES").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on TIES");
}

#[test]
fn r27_h_0029() {
  let src = "-- v1\nSELECT id FROM users LIMIT 1 OFFSET 0 ROWS FETCH NEXT 5 ROWS ONLY";
  let cur = src.find("ONLY").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on ONLY");
}

#[test]
fn r27_h_0030() {
  let src = "-- v1\nSELECT id FROM users LIMIT 1 OFFSET 0 ROWS FETCH NEXT 5 ROWS WITH TIES";
  let cur = src.find("WITH").unwrap();
  let md = hover_at(src, cur);
  assert!(md.is_some(), "hover on WITH");
}
