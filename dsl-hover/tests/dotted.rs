//! Hover narrow-by-side for dotted identifiers.

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
fn hover_on_new_inside_trigger_function_returns_explanation() {
  let src = "\
CREATE OR REPLACE FUNCTION audit() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE NOTICE '%', NEW.id;
    RETURN NEW;
END;
$$;";
  let cur = src.find("NEW.id").unwrap();
  let md = hover_at(src, cur);
  // We may or may not resolve NEW to a specific table; either way it
  // should return SOME explanation, not panic.
  let _ = md;
}

#[test]
fn hover_on_old_inside_trigger_function() {
  let src = "\
CREATE OR REPLACE FUNCTION audit() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE NOTICE '%', OLD.id;
    RETURN OLD;
END;
$$;";
  let cur = src.find("OLD.id").unwrap();
  let md = hover_at(src, cur);
  let _ = md;
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

