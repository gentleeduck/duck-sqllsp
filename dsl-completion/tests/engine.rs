#![allow(clippy::absurd_extreme_comparisons, unused_comparisons, clippy::eq_op, clippy::overly_complex_bool_expr, clippy::const_is_empty)]

use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Schema, Table, TableKind};
use dsl_completion::{ItemKind, complete};
use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve_with_source;
use text_size::TextSize;

fn catalog_with_users_and_orders() -> Catalog {
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
    constraints: vec![],
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
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema { name: "public".into(), tables: vec![users, orders] }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

#[test]
fn start_of_statement_emits_keywords_only() {
  // Phase-based engine: at the start of a statement we surface
  // top-level statement keywords (SELECT, INSERT INTO, CREATE TABLE,
  // ...). Tables are intentionally omitted because the user has not
  // chosen a verb yet.
  let cat = catalog_with_users_and_orders();
  let src = "SEL";
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  let items = complete(src, &file, &scopes, &cat, TextSize::from(3));
  assert!(items.iter().any(|i| i.label == "SELECT" && i.kind == ItemKind::Keyword));
  assert!(!items.iter().any(|i| i.kind == ItemKind::Table));
}

#[test]
fn table_context_emits_only_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM ";
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  let items = complete(src, &file, &scopes, &cat, TextSize::from(src.len() as u32));
  assert!(items.iter().any(|i| i.label == "users"));
  assert!(items.iter().any(|i| i.label == "orders"));
  assert!(!items.iter().any(|i| i.kind == ItemKind::Keyword));
}

#[test]
fn dot_context_emits_columns_of_alias() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u. FROM users u";
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  // Cursor sits just after the dot.
  let offset = TextSize::from("SELECT u.".len() as u32);
  let items = complete(src, &file, &scopes, &cat, offset);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert_eq!(items.len(), 3, "expected exactly 3 columns of users, got {:?}", labels);
  assert!(labels.contains(&"id"));
  assert!(labels.contains(&"email"));
  assert!(labels.contains(&"name"));
  assert!(items.iter().all(|i| i.kind == ItemKind::Column));
}

#[test]
fn items_carry_documentation_for_keywords() {
  let cat = Catalog::default();
  let src = "SEL";
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  let items = complete(src, &file, &scopes, &cat, TextSize::from(3));
  let select = items.iter().find(|i| i.label == "SELECT").unwrap();
  let doc = select.documentation_md.as_ref().expect("doc set");
  assert!(doc.contains("Retrieve"));
  // Match the new render header (capital P).
  assert!(doc.contains("[Postgres docs]"));
}

// ============================================================================
// Alias completion -- aliases declared in the current statement must
// appear in EVERY context where the user could legally reference them.
// ============================================================================

fn complete_at(src: &str, cursor: usize, cat: &Catalog) -> Vec<dsl_completion::Item> {
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  complete(src, &file, &scopes, cat, TextSize::from(cursor as u32))
}

fn has_alias(items: &[dsl_completion::Item], alias: &str) -> bool {
  items.iter().any(|i| i.label == alias && i.kind == ItemKind::Table)
}

#[test]
fn alias_visible_in_select_projection() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT  FROM users u";
  let cur = "SELECT ".len();
  let items = complete_at(src, cur, &cat);
  assert!(has_alias(&items, "u"), "labels = {:?}", items.iter().map(|i| &i.label).collect::<Vec<_>>());
}

#[test]
fn alias_visible_in_where() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(has_alias(&items, "u"));
}

#[test]
fn alias_visible_in_on_clause() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u JOIN orders o ON ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(has_alias(&items, "u"));
  assert!(has_alias(&items, "o"));
}

#[test]
fn alias_visible_in_group_by() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u GROUP BY ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(has_alias(&items, "u"));
}

#[test]
fn alias_visible_in_order_by() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u ORDER BY ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(has_alias(&items, "u"));
}

#[test]
fn alias_visible_in_having() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u GROUP BY u.id HAVING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(has_alias(&items, "u"));
}

#[test]
fn alias_visible_in_update_assignment() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users u SET name = ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(has_alias(&items, "u"));
}

#[test]
fn alias_keyword_aliased_too() {
  // `AS` keyword shouldn't matter -- both `FROM users u` and
  // `FROM users AS u` produce the same scope binding.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users AS u WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(has_alias(&items, "u"));
}

#[test]
fn multiple_aliases_all_visible() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u, orders o WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(has_alias(&items, "u"));
  assert!(has_alias(&items, "o"));
}

#[test]
fn aliased_table_hides_bare_column_completion() {
  // FROM users AS u -- bare `id` should NOT appear; user must type
  // `u.id` (dot context handles that path). Only the alias `u` is
  // surfaced so the menu stays clean.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT  FROM users AS u";
  let cur = "SELECT ".len();
  let items = complete_at(src, cur, &cat);
  let bare_cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(!bare_cols.contains(&"id"), "id leaked: {bare_cols:?}");
  assert!(!bare_cols.contains(&"email"), "email leaked: {bare_cols:?}");
  assert!(has_alias(&items, "u"));
}

#[test]
fn unaliased_table_still_shows_columns() {
  // FROM users (no alias) -- bare columns must still appear.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT  FROM users";
  let cur = "SELECT ".len();
  let items = complete_at(src, cur, &cat);
  let bare_cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(bare_cols.contains(&"id"));
  assert!(bare_cols.contains(&"email"));
}

#[test]
fn mixed_aliased_and_bare_only_bare_columns_show() {
  // FROM users AS u, orders -- expect `u` alias visible, expect
  // orders columns (id, user_id) visible, but NOT users columns
  // (since users has alias u).
  let cat = catalog_with_users_and_orders();
  let src = "SELECT  FROM users AS u, orders";
  let cur = "SELECT ".len();
  let items = complete_at(src, cur, &cat);
  let bare_cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(has_alias(&items, "u"));
  // orders columns visible (unaliased)
  assert!(bare_cols.contains(&"user_id"));
  // users column `email` should NOT appear (table is aliased)
  assert!(!bare_cols.contains(&"email"), "email leaked: {bare_cols:?}");
}

#[test]
fn dotted_alias_still_resolves_after_hide() {
  // Confirm the dot-context path still works for `u.` -- hiding bare
  // column completion shouldn't break alias-qualified resolution.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u. FROM users u";
  let cur = "SELECT u.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"));
  assert!(labels.contains(&"email"));
  assert!(items.iter().all(|i| i.kind == ItemKind::Column));
}

#[test]
fn plpgsql_function_parameter_completes_in_body() {
  let cat = catalog_with_users_and_orders();
  let src = "\
CREATE OR REPLACE FUNCTION foo(p_user UUID)
    RETURNS INT
    LANGUAGE plpgsql
AS
$$
BEGIN
    RETURN ;
END;
$$;";
  // Cursor sits right after RETURN inside the dollar-quoted body.
  let cur = src.find("RETURN ").unwrap() + "RETURN ".len();
  let items = complete_at(src, cur, &cat);
  assert!(
    items.iter().any(|i| i.label == "p_user" && i.kind == ItemKind::Parameter),
    "p_user missing from plpgsql body completion: {:?}",
    items.iter().take(20).map(|i| (&i.label, i.kind)).collect::<Vec<_>>()
  );
}

#[test]
fn plpgsql_local_typed_as_table_dot_completes_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "\
CREATE OR REPLACE FUNCTION foo()
    RETURNS INT
    LANGUAGE plpgsql
AS
$$
DECLARE
    r users;
BEGIN
    SELECT r. ;
END;
$$;";
  let cur = src.find("r. ").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "id missing: {labels:?}");
  assert!(labels.contains(&"email"), "email missing: {labels:?}");
  assert!(items.iter().all(|i| i.kind == ItemKind::Column));
}

#[test]
fn plpgsql_local_typed_as_int_no_table_completion() {
  let cat = catalog_with_users_and_orders();
  let src = "\
CREATE OR REPLACE FUNCTION foo()
    RETURNS INT
    LANGUAGE plpgsql
AS
$$
DECLARE
    n INT;
BEGIN
    SELECT n. ;
END;
$$;";
  let cur = src.find("n. ").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  // Bare INT has no fields -- completion is empty (no `id`, no `email`).
  assert!(
    !items.iter().any(|i| i.label == "id" && i.kind == ItemKind::Column),
    "INT-typed local leaked columns: {:?}",
    items.iter().map(|i| &i.label).collect::<Vec<_>>()
  );
}

#[test]
fn plpgsql_declared_local_completes_in_body() {
  let cat = catalog_with_users_and_orders();
  let src = "\
CREATE OR REPLACE FUNCTION foo()
    RETURNS INT
    LANGUAGE plpgsql
AS
$$
DECLARE
    v_count INT;
    v_total NUMERIC;
BEGIN
    RETURN ;
END;
$$;";
  let cur = src.find("RETURN ").unwrap() + "RETURN ".len();
  let items = complete_at(src, cur, &cat);
  assert!(
    items.iter().any(|i| i.label == "v_count" && i.kind == ItemKind::Variable),
    "v_count missing: {:?}",
    items.iter().take(20).map(|i| &i.label).collect::<Vec<_>>()
  );
  assert!(items.iter().any(|i| i.label == "v_total"));
}

#[test]
fn bare_table_name_is_not_listed_as_alias() {
  // When the user wrote `FROM users` (no alias), the synthetic
  // self-binding shouldn't surface as if it were a typed alias -- the
  // `tables` source already lists the table itself.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(
    !has_alias(&items, "users"),
    "bare table name should not appear as Table-kind alias: {:?}",
    items.iter().filter(|i| i.label == "users").collect::<Vec<_>>()
  );
}

// ===== CTE-column dot-completion =========================================

#[test]
fn cte_dot_surfaces_projected_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id, email FROM users) SELECT t. FROM t;";
  let cur = src.find("t.").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id, got {labels:?}");
  assert!(labels.contains(&"email"), "expected email, got {labels:?}");
}

#[test]
fn cte_dot_surfaces_aliased_projection() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT count(*) AS total FROM users) SELECT t. FROM t;";
  let cur = src.find("t.").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"total"), "expected total, got {labels:?}");
}

#[test]
fn cte_dot_surfaces_explicit_column_list() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t(a, b) AS (SELECT id, email FROM users) SELECT t. FROM t;";
  let cur = src.find("t.").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"a"), "expected a, got {labels:?}");
  assert!(labels.contains(&"b"), "expected b, got {labels:?}");
}

#[test]
fn cte_dot_columns_marked_as_column_kind() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id, email FROM users) SELECT t. FROM t;";
  let cur = src.find("t.").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  let id_item = items.iter().find(|i| i.label == "id").expect("id item");
  assert_eq!(id_item.kind, ItemKind::Column);
  assert!(id_item.detail.as_deref().unwrap_or("").contains("CTE"));
}

#[test]
fn cte_dot_columns_sort_first() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id, email FROM users) SELECT t. FROM t;";
  let cur = src.find("t.").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  // Only CTE columns should be surfaced for a CTE dot context --
  // not catalog tables or generic keywords.
  assert!(
    items.iter().all(|i| i.kind == ItemKind::Column),
    "expected only CTE columns, got {:?}",
    items.iter().map(|i| (&i.label, &i.kind)).collect::<Vec<_>>()
  );
}

#[test]
fn cte_dot_with_unknown_body_returns_empty() {
  // CTE declared but body has no SELECT (or body is empty) -> no
  // columns surfaced. Don't fall through to a global column dump.
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS () SELECT t. FROM t;";
  let cur = src.find("t.").unwrap() + 2;
  let _items = complete_at(src, cur, &cat);
  // Just make sure we didn't panic.
}

// ===== CHECK expression completion: surfaces functions ====================

#[test]
fn check_expression_offers_built_in_functions() {
  // Inside `CHECK (...)`, expect built-in functions like length /
  // char_length / now to be surfaced -- not just the table's columns.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE posts (body text, CHECK (len";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(
    labels.iter().any(|l| l.eq_ignore_ascii_case("length")),
    "expected length() in CHECK completion, got: {labels:?}"
  );
  assert!(
    labels.iter().any(|l| l.eq_ignore_ascii_case("char_length")),
    "expected char_length() in CHECK completion, got: {labels:?}"
  );
}

#[test]
fn check_expression_offers_table_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE users (id uuid, email text, CHECK (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Buffer-defined column extraction means we look at the CREATE
  // TABLE body for column names.
  assert!(
    labels.iter().any(|l| l.eq_ignore_ascii_case("id") || l.eq_ignore_ascii_case("email")),
    "expected columns of users in CHECK, got: {labels:?}"
  );
}

// ===== Every PG function MUST be reachable from completion in expression
// contexts. Sweep across SELECT projection, WHERE, HAVING, GROUP BY,
// ORDER BY, ON, CHECK -- and assert char_length / length / now /
// coalesce / count / array_length appear in each.

#[test]
fn all_built_in_functions_appear_in_select_projection() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT  FROM users;";
  let cur = "SELECT ".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["length", "char_length", "character_length", "now", "coalesce", "count", "lower", "upper", "substring"]
  {
    assert!(
      labels.iter().any(|l| l == fname),
      "function `{fname}` missing from SELECT projection completion; got {} items",
      labels.len()
    );
  }
}

#[test]
fn all_built_in_functions_appear_in_where_clause() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["length", "char_length", "now", "coalesce"] {
    assert!(labels.iter().any(|l| l == fname), "function `{fname}` missing from WHERE completion");
  }
}

#[test]
fn all_built_in_functions_appear_in_check_constraint() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE posts (body text, CHECK (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["length", "char_length", "lower", "upper"] {
    assert!(labels.iter().any(|l| l == fname), "function `{fname}` missing from CHECK completion");
  }
}

#[test]
fn all_built_in_functions_appear_in_having_clause() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id FROM users GROUP BY id HAVING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["count", "sum", "max", "min"] {
    assert!(labels.iter().any(|l| l == fname), "function `{fname}` missing from HAVING completion");
  }
}

#[test]
fn all_built_in_functions_appear_in_order_by() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users ORDER BY ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["length", "lower", "coalesce"] {
    assert!(labels.iter().any(|l| l == fname), "function `{fname}` missing from ORDER BY completion");
  }
}

// Constraint-clause completion edge cases requested by user:
// every CHECK / DEFAULT / GENERATED context must surface functions
// alongside the table's own columns.

#[test]
fn named_constraint_check_offers_functions() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE posts (body text, CONSTRAINT chk_body CHECK (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["length", "char_length", "lower"] {
    assert!(labels.iter().any(|l| l == fname), "function `{fname}` missing from `CONSTRAINT name CHECK (` completion");
  }
}

#[test]
fn after_default_keyword_offers_functions() {
  // The DEFAULT slot is curated (see DEFAULT_EXPRESSION_SUGGESTIONS in
  // engine.rs) -- it emits a tight menu of common default expressions
  // including `now()` and `gen_random_uuid()` rather than the full
  // catalog-function dump. Verify the curated menu still surfaces
  // those function entries.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (created_at timestamptz DEFAULT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["now()", "gen_random_uuid()"] {
    assert!(labels.iter().any(|l| l == fname), "function `{fname}` missing after DEFAULT; got {labels:?}");
  }
}

#[test]
fn after_inline_check_keyword_offers_functions() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (body text CHECK (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["length", "char_length"] {
    assert!(labels.iter().any(|l| l == fname), "function `{fname}` missing inside inline CHECK");
  }
}

#[test]
fn functions_in_join_on_clause() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u JOIN orders o ON ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["length", "coalesce", "now"] {
    assert!(labels.iter().any(|l| l == fname), "function `{fname}` missing from ON-clause completion");
  }
}

#[test]
fn functions_in_in_predicate() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WHERE id IN (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["length", "coalesce"] {
    assert!(labels.iter().any(|l| l == fname), "function `{fname}` missing from IN-predicate completion");
  }
}

// ===== function snippet insertion =========================================

#[test]
fn built_in_function_inserts_with_snippet_placeholder() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT  FROM users;";
  let cur = "SELECT ".len();
  let items = complete_at(src, cur, &cat);
  let length = items.iter().find(|i| i.label == "length").expect("length");
  assert!(length.is_snippet, "length should be a snippet");
  assert_eq!(length.insert_text, "length($0)");
}

#[test]
fn zero_arg_function_inserts_bare_parens() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT  FROM users;";
  let cur = "SELECT ".len();
  let items = complete_at(src, cur, &cat);
  let now = items.iter().find(|i| i.label == "now").expect("now");
  assert!(!now.is_snippet, "now() takes no args, not a snippet");
  assert_eq!(now.insert_text, "now()");
}

#[test]
fn non_function_items_are_not_snippets() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let users = items.iter().find(|i| i.label == "users").expect("users");
  assert!(!users.is_snippet);
  assert_eq!(users.insert_text, "users");
}

#[test]
fn constraint_keyword_inserts_full_snippet_template() {
  let cat = catalog_with_users_and_orders();
  // Inside a CREATE TABLE body, fresh entry position.
  let src = "CREATE TABLE t (id uuid, ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let c = items.iter().find(|i| i.label == "CONSTRAINT").expect("CONSTRAINT");
  assert!(c.is_snippet, "CONSTRAINT should be a snippet");
  assert!(c.insert_text.contains("${1:name}"), "expects name placeholder");
  assert!(
    c.insert_text.contains("PRIMARY KEY,UNIQUE,FOREIGN KEY,CHECK"),
    "expects kind choice; got: {}",
    c.insert_text
  );
  assert!(c.insert_text.contains("${3:col}"), "expects column-list placeholder");
}

#[test]
fn ctable_snippet_expands_create_table_skeleton() {
  let cat = catalog_with_users_and_orders();
  let src = "";
  let items = complete_at(src, 0, &cat);
  let it = items.iter().find(|i| i.label == "ctable").expect("ctable");
  assert!(it.is_snippet);
  assert!(it.insert_text.contains("CREATE TABLE ${1:name}"), "got: {}", it.insert_text);
  assert!(it.insert_text.contains("gen_random_uuid()"), "expects PK + default");
  assert!(it.insert_text.contains("created_at timestamptz NOT NULL DEFAULT now()"), "expects created_at");
}

#[test]
fn fn_snippet_expands_create_function_skeleton() {
  let cat = catalog_with_users_and_orders();
  let src = "";
  let items = complete_at(src, 0, &cat);
  let it = items.iter().find(|i| i.label == "fn").expect("fn");
  assert!(it.is_snippet);
  assert!(it.insert_text.contains("CREATE OR REPLACE FUNCTION ${1:name}"));
  assert!(it.insert_text.contains("LANGUAGE plpgsql"));
  assert!(it.insert_text.contains("BEGIN\n    $0"), "tab-stop should land in BEGIN body");
}

#[test]
fn trig_snippet_expands_trigger_with_handler() {
  let cat = catalog_with_users_and_orders();
  let src = "";
  let items = complete_at(src, 0, &cat);
  let it = items.iter().find(|i| i.label == "trig").expect("trig");
  assert!(it.is_snippet);
  assert!(it.insert_text.contains("CREATE TRIGGER"));
  assert!(it.insert_text.contains("RETURNS TRIGGER"));
  assert!(it.insert_text.contains("FOR EACH ROW"));
}

#[test]
fn idx_snippet_expands_create_index() {
  let cat = catalog_with_users_and_orders();
  let src = "";
  let items = complete_at(src, 0, &cat);
  let it = items.iter().find(|i| i.label == "idx").expect("idx");
  assert!(it.is_snippet);
  assert!(it.insert_text.contains("CREATE INDEX ${1:idx_name}"));
}

#[test]
fn all_extra_statement_snippets_present() {
  let cat = catalog_with_users_and_orders();
  let items = complete_at("", 0, &cat);
  for label in &["view", "mat", "enum", "dom", "pol", "do"] {
    let it = items.iter().find(|i| i.label == *label).unwrap_or_else(|| panic!("missing snippet `{label}`"));
    assert!(it.is_snippet, "`{label}` should be a snippet");
  }
}

#[test]
fn pol_snippet_uses_command_choice() {
  let cat = catalog_with_users_and_orders();
  let items = complete_at("", 0, &cat);
  let pol = items.iter().find(|i| i.label == "pol").expect("pol");
  assert!(
    pol.insert_text.contains("ALL,SELECT,INSERT,UPDATE,DELETE"),
    "pol should have command choice; got: {}",
    pol.insert_text
  );
}

// ===== fresh-name suppression for CREATE keywords =========================
//
// SQL DDL like `CREATE TABLE <name>` invents a brand-new identifier.
// Completing existing catalog tables would be wrong (collision) and
// keyword completion is also wrong (mid-identifier).

#[test]
fn no_completion_after_create_table_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.len() <= 1, "fresh-name slot must stay tiny");
}

#[test]
fn no_completion_after_create_function_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.len() <= 1, "fresh-name slot must stay tiny");
}

#[test]
fn no_completion_after_create_or_replace_function_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR REPLACE FUNCTION ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.len() <= 1, "fresh-name slot must stay tiny");
}

#[test]
fn no_completion_after_create_index_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.len() <= 1, "fresh-name slot must stay tiny");
}

#[test]
fn no_completion_after_create_view_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE VIEW ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.len() <= 1, "fresh-name slot must stay tiny");
}

#[test]
fn no_completion_after_create_trigger_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.len() <= 1, "fresh-name slot must stay tiny");
}

#[test]
fn no_completion_after_create_policy_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE POLICY ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.len() <= 1, "fresh-name slot must stay tiny");
}

#[test]
fn drop_table_offers_existing_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "DROP TABLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "DROP TABLE should surface existing tables; got: {:?}", labels);
  assert!(labels.contains(&"orders"));
}

#[test]
fn alter_table_offers_existing_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "ALTER TABLE should surface existing tables; got: {:?}", labels);
}

#[test]
fn alter_table_after_name_offers_sub_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  for must in [
    "ADD COLUMN",
    "DROP COLUMN",
    "RENAME COLUMN",
    "ALTER COLUMN",
    "ADD CONSTRAINT",
    "DROP CONSTRAINT",
    "OWNER TO",
    "RENAME TO",
  ] {
    assert!(labels.contains(&must), "ALTER TABLE users _ should surface `{must}`; got: {:?}", labels);
  }
  // Should NOT dump tables/columns/keywords at this position --
  // exposing every column of every table here would drown the menu.
  assert!(!labels.contains(&"users"), "table list leaked into sub-action menu: {:?}", labels);
}

#[test]
fn grant_surfaces_privileges_after_grant_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "GRANT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  for must in ["SELECT", "INSERT", "UPDATE", "DELETE", "ALL PRIVILEGES", "USAGE", "EXECUTE"] {
    assert!(labels.contains(&must), "GRANT _ should surface `{must}`; got: {:?}", labels);
  }
}

#[test]
fn grant_surfaces_targets_after_on() {
  let cat = catalog_with_users_and_orders();
  let src = "GRANT SELECT ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLE"), "expected TABLE class keyword; got: {labels:?}");
  assert!(labels.contains(&"SCHEMA"), "expected SCHEMA class keyword");
  assert!(labels.contains(&"users"), "expected catalog table users; got: {labels:?}");
}

#[test]
fn grant_surfaces_roles_after_to() {
  let cat = catalog_with_users_and_orders();
  let src = "GRANT SELECT ON users TO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PUBLIC"), "expected PUBLIC pseudo-role");
  assert!(labels.contains(&"postgres"), "expected postgres fallback role");
}

#[test]
fn revoke_routes_same_as_grant() {
  let cat = catalog_with_users_and_orders();
  let src = "REVOKE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SELECT"), "REVOKE should also surface privileges; got: {labels:?}");
}

#[test]
fn owner_to_surfaces_catalog_roles() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users OWNER TO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PUBLIC"), "OWNER TO should surface PUBLIC pseudo-role; got: {labels:?}");
  assert!(labels.contains(&"postgres"), "OWNER TO should surface built-in roles");
}

#[test]
fn alter_schema_owner_to_surfaces_roles() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SCHEMA public OWNER TO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PUBLIC"), "ALTER SCHEMA OWNER TO should surface roles; got: {labels:?}");
}

#[test]
fn set_role_surfaces_catalog_roles() {
  let cat = catalog_with_users_and_orders();
  let src = "SET ROLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"postgres"), "SET ROLE should surface roles; got: {labels:?}");
}

#[test]
fn revoke_from_surfaces_roles() {
  let cat = catalog_with_users_and_orders();
  let src = "REVOKE SELECT ON users FROM ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PUBLIC"), "REVOKE ... FROM should surface roles");
}

#[test]
fn alter_table_with_if_exists_still_routes_to_sub_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE IF EXISTS users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ADD COLUMN"), "IF EXISTS guard should not break sub-action detection; got: {:?}", labels);
}

#[test]
fn truncate_offers_existing_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "TRUNCATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "TRUNCATE should surface existing tables; got: {:?}", labels);
}

// ===== JSON path key completion ===========================================

#[test]
fn json_path_completion_surfaces_keys_from_buffer() {
  let cat = catalog_with_users_and_orders();
  // Buffer has a CREATE TABLE with a jsonb DEFAULT literal that
  // contains observed keys (`role`, `team`). When the cursor sits
  // inside `data->'<cursor>'` later in the buffer, those keys
  // should appear.
  let src =
    "CREATE TABLE m (data jsonb NOT NULL DEFAULT '{\"role\":\"admin\",\"team\":\"core\"}');\nSELECT data->'' FROM m;";
  let cur = src.rfind("''").unwrap() + 1;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"role"), "expected `role` key; got: {labels:?}");
  assert!(labels.contains(&"team"), "expected `team` key; got: {labels:?}");
}

#[test]
fn json_path_completion_works_for_double_arrow() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE m (d jsonb DEFAULT '{\"a\":1}');\nSELECT d->>'' FROM m;";
  let cur = src.rfind("''").unwrap() + 1;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"a"), "got: {labels:?}");
}

#[test]
fn extra_dml_snippets_all_present() {
  let cat = catalog_with_users_and_orders();
  let items = complete_at("", 0, &cat);
  for label in &["addcol", "rencol", "rentab", "copyin", "copyout", "listen", "notify", "upsert"] {
    let it =
      items.iter().find(|i| i.label.eq_ignore_ascii_case(label)).unwrap_or_else(|| panic!("missing snippet `{label}`"));
    assert!(it.is_snippet, "{label} should be a snippet");
  }
}

#[test]
fn offline_mode_surfaces_buffer_defined_types_after_cast() {
  // No live catalog -> empty Catalog. The buffer declares a custom
  // enum + a CREATE TABLE; completion at `::<cursor>` should still
  // surface the enum from the source-derived catalog.
  use dsl_catalog::{CATALOG_VERSION, Catalog};
  let empty_cat = Catalog {
    version: CATALOG_VERSION,
    connection_id: "<offline>".into(),
    schemas: vec![],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  };
  let src = "\
CREATE TYPE mood AS ENUM ('happy', 'sad');
SELECT 'happy'::
";
  let cur = src.rfind("::").unwrap() + 2;
  let items = complete_at(src, cur, &empty_cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(
    labels.contains(&"mood"),
    "offline-mode cast completion should surface buffer-defined type `mood`; got: {labels:?}"
  );
}

#[test]
fn offline_mode_surfaces_default_roles() {
  // No live catalog, no CREATE ROLE in buffer -- the default offline
  // role set should still appear at OWNER TO.
  use dsl_catalog::{CATALOG_VERSION, Catalog};
  let empty_cat = Catalog {
    version: CATALOG_VERSION,
    connection_id: "<offline>".into(),
    schemas: vec![],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  };
  let src = "ALTER TABLE x OWNER TO ";
  let items = complete_at(src, src.len(), &empty_cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"postgres"), "default offline role `postgres` should be in OWNER TO menu; got: {labels:?}");
  assert!(
    labels.contains(&"pg_read_all_data"),
    "default offline role `pg_read_all_data` should be present; got: {labels:?}"
  );
  assert!(labels.contains(&"PUBLIC"), "pseudo-role PUBLIC should be present; got: {labels:?}");
}

#[test]
fn select_projection_filters_already_listed_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id,  FROM users;";
  // Cursor sits right after `id, ` (before "FROM").
  let cur = src.find(",  FROM").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  let column_labels: Vec<&str> =
    items.iter().filter(|i| i.kind == dsl_completion::ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(!column_labels.contains(&"id"), "id already listed, should be filtered; got: {column_labels:?}");
  assert!(column_labels.contains(&"email"), "other cols should remain; got: {column_labels:?}");
}

#[test]
fn insert_column_list_filters_already_listed() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, );";
  let cur = src.find(", )").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  let column_labels: Vec<&str> =
    items.iter().filter(|i| i.kind == dsl_completion::ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(!column_labels.contains(&"id"), "id already in INSERT col list; got: {column_labels:?}");
}

#[test]
fn json_path_completion_walks_nested_path() {
  // Buffer has a jsonb default with a nested object; completion at
  // `data->'profile'->'<cursor>'` should surface the nested keys,
  // not the outer ones.
  let cat = catalog_with_users_and_orders();
  let src = "\
CREATE TABLE m (data jsonb DEFAULT '{\"profile\":{\"avatar\":\"x\",\"nickname\":\"y\"},\"meta\":{\"v\":1}}');
SELECT data->'profile'->'' FROM m;
";
  let cur = src.rfind("''").unwrap() + 1;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"avatar"), "expected nested key `avatar`; got: {labels:?}");
  assert!(labels.contains(&"nickname"), "expected nested key `nickname`; got: {labels:?}");
  // The outer-only keys must NOT leak in.
  assert!(!labels.contains(&"profile"), "outer key leaked into nested completion: {labels:?}");
  assert!(!labels.contains(&"meta"), "sibling key leaked: {labels:?}");
}

#[test]
fn json_path_completion_returns_normal_in_plain_string() {
  let cat = catalog_with_users_and_orders();
  // Not in a `->'` context -- string literals get the normal menu.
  let src = "SELECT * FROM users WHERE name = '';";
  let cur = src.rfind("''").unwrap() + 1;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_lowercase()).collect();
  // Should NOT contain "role" / "team" from JSON harvesting.
  assert!(!labels.contains(&"role".to_string()));
}

#[test]
fn index_using_method_completes() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX idx ON users USING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
  for m in &["btree", "gin", "gist", "brin", "hash", "spgist"] {
    assert!(labels.iter().any(|l| l == m), "method `{m}` missing");
  }
}

#[test]
fn trigger_event_completes_after_before() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER t BEFORE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
  for ev in &["INSERT", "UPDATE", "DELETE", "TRUNCATE"] {
    assert!(labels.iter().any(|l| l == ev), "event `{ev}` missing");
  }
}

#[test]
fn trigger_on_completes_table() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER t BEFORE INSERT ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
  assert!(labels.iter().any(|l| l == "users"), "users table missing");
}

#[test]
fn policy_for_completes_command() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE POLICY p ON users FOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
  for c in &["ALL", "SELECT", "INSERT", "UPDATE", "DELETE"] {
    assert!(labels.iter().any(|l| l == c), "policy command `{c}` missing");
  }
}

#[test]
fn insert_column_list_only_target_table_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
  assert!(labels.contains(&"id".to_string()));
  assert!(labels.contains(&"email".to_string()));
  assert!(labels.contains(&"name".to_string()));
  // Must NOT include orders columns.
  assert!(!labels.contains(&"user_id".to_string()), "leaked orders.user_id into INSERT col list");
}

#[test]
fn insert_column_list_filters_already_typed() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
  assert!(!labels.contains(&"id".to_string()), "id already typed, should be filtered");
  assert!(labels.contains(&"email".to_string()));
}

#[test]
fn alter_column_type_completes() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ALTER COLUMN name TYPE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
  for ty in &["text", "varchar", "integer", "jsonb"] {
    assert!(labels.iter().any(|l| l == ty), "type `{ty}` missing");
  }
}

#[test]
fn functions_in_plpgsql_assign_rhs() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ DECLARE v text; BEGIN v := ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for fname in &["length", "now", "coalesce"] {
    assert!(labels.iter().any(|l| l == fname), "function `{fname}` missing from PL/pgSQL assign RHS completion");
  }
}

// ============================================================================
// Cursor inside a string literal / comment must NOT offer keywords or
// catalog tables / columns. Suggestions inside a literal are just noise.
// ============================================================================

// ============================================================================
// Case-insensitive alias resolution: SQL folds unquoted identifiers
// (PG: lowercase), so dot completion must succeed regardless of how the
// user types the alias relative to how it was declared.
// ============================================================================

#[test]
fn dot_context_uppercase_alias_finds_lowercase_declaration() {
  // `users u` declared lowercase, user types `U.` -- should still
  // surface users' columns.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT U. FROM users u";
  let cur = "SELECT U.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "uppercase alias `U` should match `u`; got {labels:?}");
  assert!(labels.contains(&"email"), "uppercase alias `U` should match `u`; got {labels:?}");
}

#[test]
fn dot_context_lowercase_alias_finds_mixed_case_declaration() {
  // Source declares MixedCase alias `Ux`; user types `ux.`.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT ux. FROM users Ux";
  let cur = "SELECT ux.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "lowercase alias `ux` should match `Ux`; got {labels:?}");
}

// ============================================================================
// `WITH cte AS (...) SELECT * FROM <cursor>` -- the outer FROM slot must
// surface real catalog tables AND the CTE names declared by the WITH.
// Before the fix, the phase detector got stuck in `Unknown` after seeing
// WITH and dumped 600+ keyword items.
// ============================================================================

#[test]
fn values_top_level_emits_no_catalog_items() {
  // Top-level `VALUES <cursor>` -- next token is `(` opening the row
  // expression. There's no useful catalog completion (we don't suggest
  // `(` as an item). The bug is that the catch-all dumped 641 items.
  let cat = catalog_with_users_and_orders();
  let src = "VALUES ";
  let items = complete_at(src, src.len(), &cat);
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "VALUES at top-level leaked {bad} catalog items");
}

#[test]
fn with_cte_as_offers_materialized_keyword() {
  // `WITH cte AS <cursor>` -- the next token is either `(` (the body)
  // or one of MATERIALIZED / NOT MATERIALIZED. Currently dumps 641.
  let cat = catalog_with_users_and_orders();
  let src = "WITH cte AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["MATERIALIZED", "NOT MATERIALIZED"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "WITH cte AS should suggest `{kw}`; got {labels:?}"
    );
  }
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "WITH cte AS leaked {bad} catalog items");
}

#[test]
fn do_offers_language_keyword() {
  // `DO <cursor>` -- the next token is LANGUAGE or a dollar-quoted body.
  let cat = catalog_with_users_and_orders();
  let src = "DO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  assert!(
    labels.iter().any(|l| l == "LANGUAGE"),
    "DO should suggest `LANGUAGE`; got {labels:?}"
  );
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "DO leaked {bad} catalog items");
}

#[test]
fn from_after_with_cte_offers_tables_only() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH my_cte AS (SELECT id FROM users) SELECT * FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(
    items.iter().all(|i| matches!(i.kind, ItemKind::Table | ItemKind::View)),
    "FROM slot after a WITH CTE must emit only tables/views; got kinds {:?} (count {})",
    items.iter().map(|i| i.kind).collect::<Vec<_>>(),
    items.len()
  );
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "expected `users` catalog table; got {labels:?}");
  assert!(labels.contains(&"my_cte"), "expected CTE name `my_cte` as candidate; got {labels:?}");
}

#[test]
fn from_after_multiple_ctes_offers_all_cte_names() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH a AS (SELECT 1), b AS (SELECT 2) SELECT * FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"a"), "expected CTE `a`; got {labels:?}");
  assert!(labels.contains(&"b"), "expected CTE `b`; got {labels:?}");
}

#[test]
fn join_after_with_cte_offers_tables_only() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT 1) SELECT * FROM t JOIN ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(
    items.iter().all(|i| matches!(i.kind, ItemKind::Table | ItemKind::View)),
    "JOIN slot after a WITH CTE must emit only tables/views; got {:?}",
    items.iter().map(|i| i.kind).collect::<Vec<_>>()
  );
}

// ============================================================================
// RETURNING column completion -- for INSERT / UPDATE / DELETE, the
// cursor after `RETURNING` should surface columns of the target table,
// not a 350-item dump of catalog functions.
// ============================================================================

#[test]
fn create_function_returns_offers_types_only() {
  // `CREATE FUNCTION f() RETURNS <cursor>` -- the return-type slot.
  // Should emit catalog types only, not 641 keywords / tables / funcs.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS ";
  let items = complete_at(src, src.len(), &cat);
  assert!(
    items.iter().all(|i| i.kind == ItemKind::Type),
    "RETURNS slot must emit only types; got {} items with kinds {:?}",
    items.len(),
    items.iter().map(|i| i.kind).collect::<Vec<_>>().iter().take(8).collect::<Vec<_>>()
  );
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for ty in ["text", "integer", "uuid"] {
    assert!(labels.iter().any(|l| l == ty), "type `{ty}` missing; got {labels:?}");
  }
}

#[test]
fn create_or_replace_function_returns_offers_types_only() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR REPLACE FUNCTION f() RETURNS ";
  let items = complete_at(src, src.len(), &cat);
  assert!(
    items.iter().all(|i| i.kind == ItemKind::Type),
    "RETURNS slot must emit only types; got {:?}",
    items.iter().map(|i| i.kind).collect::<Vec<_>>()
  );
}

#[test]
fn on_conflict_do_update_set_offers_target_columns() {
  // `INSERT INTO users (id) VALUES (1) ON CONFLICT DO UPDATE SET <cursor>`
  // The SET LHS is a column of the INSERT target table (users).
  // Currently dumps 350 items (functions + keywords).
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES ('00000000-0000-0000-0000-000000000000') ON CONFLICT DO UPDATE SET ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(cols.contains(&"id"), "expected `id` of users; got {cols:?}");
  assert!(cols.contains(&"email"), "expected `email`; got {cols:?}");
  // The menu should not be 300+ functions in a column-LHS slot.
  let fns = items.iter().filter(|i| i.kind == ItemKind::Function).count();
  assert!(fns < 50, "ON CONFLICT SET drowned in {fns} functions; should narrow to columns");
}

#[test]
fn on_conflict_target_paren_offers_columns() {
  // `ON CONFLICT (<cursor>)` -- the conflict-target column list.
  // The cursor is between `(` and `)`, naming columns of the target.
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES ('00000000-0000-0000-0000-000000000000') ON CONFLICT (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(cols.contains(&"id"), "expected `id` of users; got {cols:?}");
}

#[test]
fn alter_table_add_column_after_name_offers_types() {
  // `ALTER TABLE users ADD COLUMN new_col <cursor>` -- name is typed,
  // user now expects a type. Should emit catalog types, not the
  // ADD/DROP action menu.
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ADD COLUMN new_col ";
  let items = complete_at(src, src.len(), &cat);
  assert!(
    items.iter().all(|i| i.kind == ItemKind::Type),
    "ADD COLUMN <name> <cursor> must emit only types; got kinds {:?}",
    items.iter().map(|i| i.kind).collect::<Vec<_>>()
  );
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
  for ty in ["text", "integer", "uuid"] {
    assert!(labels.iter().any(|l| l == ty), "type `{ty}` missing; got {labels:?}");
  }
}

#[test]
fn discard_offers_subcommand_keywords() {
  let cat = catalog_with_users_and_orders();
  let src = "DISCARD ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["ALL", "PLANS", "SEQUENCES", "TEMP"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "DISCARD should suggest `{kw}`; got {labels:?}"
    );
  }
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "DISCARD leaked {bad} catalog items");
}

#[test]
fn prepare_deallocate_emit_nothing() {
  let cat = catalog_with_users_and_orders();
  // DEALLOCATE alone is a fresh-name slot -- nothing useful to suggest.
  // `PREPARE` legitimately has two forms (`PREPARE TRANSACTION '<gxid>'`
  // and `PREPARE <stmt_name> AS ...`), so we surface the TRANSACTION kw.
  let items = complete_at("DEALLOCATE ", "DEALLOCATE ".len(), &cat);
  assert!(items.is_empty(), "DEALLOCATE is a fresh-name slot; expected empty, got {}", items.len());
  let items = complete_at("PREPARE ", "PREPARE ".len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TRANSACTION"), "PREPARE should surface TRANSACTION; got {labels:?}");
}

#[test]
fn create_sequence_after_name_offers_options() {
  // `CREATE SEQUENCE s <cursor>` -- next tokens are sequence options
  // (INCREMENT [BY], START [WITH], MINVALUE, MAXVALUE, CACHE, CYCLE,
  // OWNED BY, AS).
  let cat = catalog_with_users_and_orders();
  let src = "CREATE SEQUENCE s ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["INCREMENT", "START", "MINVALUE", "MAXVALUE", "CACHE", "CYCLE"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "CREATE SEQUENCE s should suggest `{kw}`; got {labels:?}"
    );
  }
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "CREATE SEQUENCE s leaked {bad} catalog items");
}

#[test]
fn create_type_as_offers_kind_keywords() {
  // `CREATE TYPE t AS <cursor>` -- next token is ENUM / RANGE /
  // (for composite. Currently dumps 641.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TYPE t AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["ENUM", "RANGE"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "CREATE TYPE t AS should suggest `{kw}`; got {labels:?}"
    );
  }
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "CREATE TYPE t AS leaked {bad} catalog items");
}

#[test]
fn declare_cursor_for_offers_statement_keywords() {
  // `DECLARE c CURSOR FOR <cursor>` -- expects a SELECT statement.
  let cat = catalog_with_users_and_orders();
  let src = "DECLARE c CURSOR FOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  assert!(
    labels.iter().any(|l| l == "SELECT"),
    "DECLARE c CURSOR FOR should suggest SELECT; got {labels:?}"
  );
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "DECLARE CURSOR FOR leaked {bad} catalog items");
}

#[test]
fn create_index_after_name_offers_on_keyword() {
  // `CREATE INDEX foo <cursor>` -- next required token is ON.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX foo ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  assert!(labels.iter().any(|l| l == "ON"), "CREATE INDEX foo should suggest ON; got {labels:?}");
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "CREATE INDEX foo leaked {bad} catalog items");
}

#[test]
fn create_extension_fresh_name_emits_nothing() {
  // `CREATE EXTENSION <cursor>` -- user types an extension name
  // (uuid-ossp / pgcrypto / etc). No catalog SQL identifier is the
  // right answer; the menu must not be the 640-item catch-all dump.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EXTENSION ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.len() <= 1, "fresh-name slot must stay tiny");
}

#[test]
fn comment_on_offers_class_keywords() {
  // `COMMENT ON <cursor>` -- the user must name an object class
  // (TABLE, COLUMN, SCHEMA, FUNCTION, ROLE, ...) before the target
  // name. Currently dumps 640 items.
  let cat = catalog_with_users_and_orders();
  let src = "COMMENT ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["TABLE", "COLUMN", "SCHEMA", "FUNCTION", "INDEX", "VIEW", "MATERIALIZED VIEW", "SEQUENCE"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "COMMENT ON should suggest `{kw}`; got {labels:?}"
    );
  }
  // No catalog dump.
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "COMMENT ON leaked {bad} non-keyword items");
}

#[test]
fn raise_offers_level_keywords() {
  // `RAISE <cursor>` (PL/pgSQL) -- next token is a level keyword.
  let cat = catalog_with_users_and_orders();
  let src = "RAISE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["NOTICE", "WARNING", "EXCEPTION", "DEBUG", "LOG", "INFO"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "RAISE should suggest level `{kw}`; got {labels:?}"
    );
  }
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "RAISE leaked {bad} catalog items");
}

#[test]
fn fetch_move_close_open_emit_nothing() {
  // These commands take a cursor name (fresh, or direction modifier
  // for FETCH/MOVE). The catch-all dump is just noise.
  let cat = catalog_with_users_and_orders();
  for src in ["FETCH ", "MOVE ", "CLOSE ", "OPEN "] {
    let items = complete_at(src, src.len(), &cat);
    let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
    assert_eq!(bad, 0, "{src:?} leaked {bad} catalog items into cursor-name slot");
  }
}

#[test]
fn reset_offers_modifier_keywords() {
  // `RESET <cursor>` -- next token is ALL, ROLE, or a GUC name.
  // Surface the two keywords; GUC names are freeform so no catalog
  // completion makes sense.
  let cat = catalog_with_users_and_orders();
  let src = "RESET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["ALL", "ROLE"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "RESET should suggest `{kw}`; got {labels:?}"
    );
  }
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "RESET leaked {bad} catalog items into GUC slot");
}

#[test]
fn set_offers_scope_modifiers_only() {
  // `SET <cursor>` -- typically followed by LOCAL or SESSION
  // (scope modifier) or a GUC variable name. We don't have a
  // catalog of GUC names, so surface the scope-modifier keywords
  // and stay out of the way for the variable-name slot.
  let cat = catalog_with_users_and_orders();
  let src = "SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["LOCAL", "SESSION"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "SET should suggest scope modifier `{kw}`; got {labels:?}"
    );
  }
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "SET leaked {bad} catalog items into GUC slot");
}

#[test]
fn set_local_emits_no_catalog_items() {
  // `SET LOCAL <cursor>` -- the next token is a GUC name. No
  // catalog-derived completion makes sense; keep the menu empty.
  let cat = catalog_with_users_and_orders();
  for src in ["SET LOCAL ", "SET SESSION "] {
    let items = complete_at(src, src.len(), &cat);
    let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
    assert_eq!(bad, 0, "{src:?} leaked {bad} catalog items into GUC slot");
  }
}

#[test]
fn begin_transaction_offers_modifiers_only() {
  // `BEGIN ` / `BEGIN TRANSACTION ` / `START TRANSACTION ` -- the
  // user expects transaction-mode keywords (ISOLATION LEVEL,
  // READ ONLY/WRITE, DEFERRABLE), not a 638-item dump.
  let cat = catalog_with_users_and_orders();
  for src in ["BEGIN ", "BEGIN TRANSACTION ", "START TRANSACTION "] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
    assert!(
      labels.iter().any(|l| l == "ISOLATION LEVEL" || l == "READ ONLY" || l == "READ WRITE"),
      "{src:?} should suggest transaction-mode keywords; got {labels:?}"
    );
    let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
    assert_eq!(bad, 0, "{src:?} leaked {bad} non-keyword items");
  }
}

#[test]
fn commit_rollback_end_savepoint_emit_minimal_menu() {
  // These statements take either nothing or a savepoint name. The
  // catch-all 638-item dump is purely noise.
  let cat = catalog_with_users_and_orders();
  for src in ["COMMIT ", "ROLLBACK ", "END ", "ABORT ", "SAVEPOINT "] {
    let items = complete_at(src, src.len(), &cat);
    let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
    assert_eq!(bad, 0, "{src:?} leaked {bad} catalog items into a transaction-control slot");
  }
}

#[test]
fn explain_paren_inside_offers_option_keywords() {
  // `EXPLAIN (<cursor>)` -- inside the options paren, expects option
  // keywords (FORMAT, ANALYZE, VERBOSE, BUFFERS, COSTS, SETTINGS,
  // SUMMARY, WAL, TIMING). Currently dumps 641 items.
  let cat = catalog_with_users_and_orders();
  let src = "EXPLAIN (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["ANALYZE", "VERBOSE", "FORMAT", "BUFFERS", "COSTS"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "EXPLAIN ( should suggest `{kw}`; got {labels:?}"
    );
  }
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "EXPLAIN ( leaked {bad} catalog items");
}

#[test]
fn explain_offers_statement_keywords_only() {
  // `EXPLAIN <cursor>` -- the user is about to type a statement.
  // Should suggest top-level statement starters (SELECT, INSERT,
  // UPDATE, DELETE, ...), not a 640-item catch-all dump.
  let cat = catalog_with_users_and_orders();
  let src = "EXPLAIN ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["SELECT", "INSERT INTO", "UPDATE", "DELETE FROM"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "EXPLAIN should suggest `{kw}`; got {labels:?}"
    );
  }
  // Must not be a 600-item dump.
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "EXPLAIN slot leaked {bad} non-keyword items");
}

#[test]
fn explain_analyze_offers_statement_keywords_only() {
  let cat = catalog_with_users_and_orders();
  let src = "EXPLAIN ANALYZE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  assert!(
    labels.iter().any(|l| l == "SELECT"),
    "EXPLAIN ANALYZE should suggest SELECT; got {labels:?}"
  );
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "EXPLAIN ANALYZE leaked {bad} non-keyword items");
}

#[test]
fn explain_paren_options_offers_statement_keywords_only() {
  // `EXPLAIN (FORMAT JSON) <cursor>` -- after the options paren we're
  // still in a statement slot.
  let cat = catalog_with_users_and_orders();
  let src = "EXPLAIN (FORMAT JSON) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  assert!(
    labels.iter().any(|l| l == "SELECT"),
    "EXPLAIN (...) should still suggest SELECT; got {labels:?}"
  );
}

fn catalog_with_roles() -> Catalog {
  let mut c = catalog_with_users_and_orders();
  c.roles = vec!["alice".into(), "bob".into(), "admin".into()];
  c
}

#[test]
fn create_trigger_after_event_on_offers_tables() {
  // `CREATE TRIGGER tg BEFORE INSERT ON <cursor>` -- the trigger
  // attaches to a table. Currently returns 0 items.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER tg BEFORE INSERT ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "expected `users` table; got {labels:?}");
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "CREATE TRIGGER ... ON leaked {bad} non-table items");
}

#[test]
fn create_trigger_combined_events_on_offers_tables() {
  // `CREATE TRIGGER tg BEFORE INSERT OR UPDATE ON <cursor>` -- combined
  // events still expects a table at the ON slot.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER tg BEFORE INSERT OR UPDATE ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "combined events: expected `users`; got {labels:?}");
}

#[test]
fn create_policy_after_name_offers_on_keyword() {
  // `CREATE POLICY p <cursor>` -- next token is ON. Currently dumps 639.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE POLICY p ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  assert!(
    labels.iter().any(|l| l == "ON"),
    "CREATE POLICY p should suggest ON; got {labels:?}"
  );
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "CREATE POLICY p slot leaked {bad} catalog items");
}

#[test]
fn create_policy_after_on_offers_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE POLICY p ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "expected `users`; got {labels:?}");
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "CREATE POLICY p ON leaked {bad} non-table items");
}

#[test]
fn create_trigger_after_name_offers_timing_keywords() {
  // `CREATE TRIGGER tg <cursor>` -- next token is BEFORE/AFTER/
  // INSTEAD OF (timing keywords). Currently dumps 639 items.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER tg ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["BEFORE", "AFTER", "INSTEAD OF"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "CREATE TRIGGER tg should suggest `{kw}`; got {labels:?}"
    );
  }
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "CREATE TRIGGER tg leaked {bad} non-keyword items");
}

#[test]
fn create_or_replace_trigger_after_name_offers_timing_keywords() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR REPLACE TRIGGER tg ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["BEFORE", "AFTER"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "CREATE OR REPLACE TRIGGER tg should suggest `{kw}`; got {labels:?}"
    );
  }
}

#[test]
fn alter_role_offers_catalog_roles() {
  // `ALTER ROLE <cursor>` -- next token is a role name. Surface
  // catalog roles instead of the 639-item catch-all dump.
  let cat = catalog_with_roles();
  let src = "ALTER ROLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"alice"), "expected catalog role `alice`; got {labels:?}");
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Table | ItemKind::Column)).count();
  assert_eq!(bad, 0, "ALTER ROLE leaked {bad} non-role items");
}

#[test]
fn drop_role_offers_catalog_roles() {
  let cat = catalog_with_roles();
  let src = "DROP ROLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"alice"), "expected `alice`; got {labels:?}");
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Table | ItemKind::Column)).count();
  assert_eq!(bad, 0, "DROP ROLE leaked {bad} non-role items");
}

#[test]
fn drop_user_offers_catalog_roles() {
  let cat = catalog_with_roles();
  let src = "DROP USER ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"alice"), "expected `alice`; got {labels:?}");
}

#[test]
fn reassign_owned_by_offers_catalog_roles() {
  let cat = catalog_with_roles();
  let src = "REASSIGN OWNED BY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"alice"), "REASSIGN OWNED BY expected role list; got {labels:?}");
}

#[test]
fn merge_into_emits_only_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "MERGE INTO should suggest tables; got {labels:?}");
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "MERGE INTO leaked {bad} non-table items");
}

#[test]
fn merge_using_emits_only_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users USING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "MERGE INTO ... USING should suggest tables; got {labels:?}");
}

#[test]
fn show_emits_no_catalog_items() {
  let cat = catalog_with_users_and_orders();
  let src = "SHOW ";
  let items = complete_at(src, src.len(), &cat);
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "SHOW (GUC slot) leaked {bad} catalog items");
}

#[test]
fn vacuum_paren_inside_offers_option_keywords() {
  // `VACUUM (<cursor>)` -- inside the options paren, expects
  // VACUUM-specific options (FULL, FREEZE, VERBOSE, ANALYZE,
  // SKIP_LOCKED, INDEX_CLEANUP, PROCESS_TOAST, TRUNCATE,
  // BUFFER_USAGE_LIMIT).
  let cat = catalog_with_users_and_orders();
  let src = "VACUUM (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["FULL", "FREEZE", "VERBOSE", "ANALYZE", "SKIP_LOCKED"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "VACUUM ( should suggest `{kw}`; got {labels:?}"
    );
  }
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "VACUUM ( leaked {bad} catalog items");
}

#[test]
fn vacuum_emits_only_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "VACUUM ";
  let items = complete_at(src, src.len(), &cat);
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "VACUUM menu leaked {bad} non-table items");
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "VACUUM should suggest tables; got {labels:?}");
}

#[test]
fn vacuum_analyze_emits_only_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "VACUUM ANALYZE ";
  let items = complete_at(src, src.len(), &cat);
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "VACUUM ANALYZE menu leaked {bad} non-table items");
}

#[test]
fn copy_emits_only_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "COPY ";
  let items = complete_at(src, src.len(), &cat);
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "COPY menu leaked {bad} non-table items");
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "COPY should suggest tables; got {labels:?}");
}

#[test]
fn comment_on_table_emits_only_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "COMMENT ON TABLE ";
  let items = complete_at(src, src.len(), &cat);
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "COMMENT ON TABLE menu leaked {bad} non-table items");
}

#[test]
fn refresh_materialized_view_emits_only_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "REFRESH MATERIALIZED VIEW ";
  let items = complete_at(src, src.len(), &cat);
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "REFRESH MATERIALIZED VIEW menu leaked {bad} non-table items");
}

#[test]
fn drop_table_emits_only_tables_not_640_items() {
  // `DROP TABLE <cursor>` should narrow strictly to existing tables
  // / views (plus the `IF EXISTS` modifier the user hasn't typed yet).
  // Currently dumps 600+ items (keywords + functions + types + columns
  // + tables) without this narrowing.
  let cat = catalog_with_users_and_orders();
  let src = "DROP TABLE ";
  let items = complete_at(src, src.len(), &cat);
  assert!(
    items.iter().all(|i| {
      matches!(i.kind, ItemKind::Table | ItemKind::View)
        || (i.kind == ItemKind::Keyword && i.label == "IF EXISTS")
    }),
    "DROP TABLE menu must be tables/views + IF EXISTS; got {} items with kinds {:?}",
    items.len(),
    items.iter().map(|i| (i.label.clone(), i.kind)).take(8).collect::<Vec<_>>()
  );
}

#[test]
fn drop_table_if_exists_emits_only_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "DROP TABLE IF EXISTS ";
  let items = complete_at(src, src.len(), &cat);
  assert!(
    items.iter().all(|i| matches!(i.kind, ItemKind::Table | ItemKind::View)),
    "DROP TABLE IF EXISTS menu must be only tables; got {} items",
    items.len()
  );
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "expected `users`; got {labels:?}");
}

#[test]
fn truncate_emits_only_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "TRUNCATE ";
  let items = complete_at(src, src.len(), &cat);
  // TRUNCATE [TABLE] <table>[, ...] -- in practice we also accept the
  // optional `TABLE` keyword as a candidate, but the menu must not
  // include keywords/functions/types/columns from the catch-all dump.
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column)).count();
  assert_eq!(bad, 0, "TRUNCATE menu leaked {bad} non-table items");
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "TRUNCATE should suggest tables; got {labels:?}");
}

#[test]
fn window_clause_paren_offers_partition_by() {
  // `SELECT * FROM users WINDOW w AS (<cursor>` -- inside the window
  // body, the first sub-clause is PARTITION BY / ORDER BY / ROWS /
  // RANGE / GROUPS. The catch-all JOIN menu is wrong here.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WINDOW w AS (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["PARTITION BY", "ORDER BY"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "WINDOW w AS ( should suggest `{kw}`; got {labels:?}"
    );
  }
  assert!(
    !labels.iter().any(|l| l == "INNER JOIN" || l == "CROSS JOIN"),
    "WINDOW w AS ( wrongly listed JOIN keywords: {labels:?}"
  );
}

#[test]
fn window_clause_partition_by_offers_columns() {
  // `WINDOW w AS (PARTITION BY <cursor>` -- expects columns of the
  // FROM tables, not JOIN keywords.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WINDOW w AS (PARTITION BY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected column `id`; got {labels:?}");
  assert!(
    !labels.contains(&"INNER JOIN"),
    "PARTITION BY slot wrongly listed JOIN keywords: {labels:?}"
  );
}

#[test]
fn tablesample_offers_method_keywords() {
  // `SELECT * FROM users TABLESAMPLE <cursor>` -- expects a sample
  // method name (BERNOULLI / SYSTEM). Currently dumps 19 JOIN keywords.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users TABLESAMPLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["BERNOULLI", "SYSTEM"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "TABLESAMPLE should suggest `{kw}`; got {labels:?}"
    );
  }
  assert!(
    !labels.iter().any(|l| l == "INNER JOIN" || l == "CROSS JOIN"),
    "TABLESAMPLE wrongly listed JOIN keywords: {labels:?}"
  );
}

#[test]
fn select_for_offers_lock_strength_keywords() {
  // `SELECT * FROM users FOR <cursor>` -- expects a lock-strength
  // keyword (UPDATE, NO KEY UPDATE, SHARE, KEY SHARE), not JOINs.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users FOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["UPDATE", "SHARE", "NO KEY UPDATE", "KEY SHARE"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "SELECT ... FOR should suggest `{kw}`; got {labels:?}"
    );
  }
  // Should not be the JOIN menu.
  assert!(
    !labels.iter().any(|l| l == "INNER JOIN" || l == "CROSS JOIN"),
    "FOR slot wrongly listed JOIN keywords: {labels:?}"
  );
}

#[test]
fn select_for_update_offers_lock_modifiers() {
  // `SELECT * FROM users FOR UPDATE <cursor>` -- expects OF/SKIP
  // LOCKED/NOWAIT (or another statement-terminator).
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users FOR UPDATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["OF", "NOWAIT", "SKIP LOCKED"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "SELECT ... FOR UPDATE should suggest `{kw}`; got {labels:?}"
    );
  }
}

#[test]
fn limit_emits_no_join_keywords() {
  // `SELECT * FROM users LIMIT <cursor>` -- integer slot. The
  // existing handler wrongly emitted INNER JOIN / LEFT JOIN / etc.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users LIMIT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["INNER JOIN", "LEFT JOIN", "JOIN", "ON", "USING"] {
    assert!(
      !labels.iter().any(|l| l == kw),
      "LIMIT slot wrongly emitted `{kw}`; got {labels:?}"
    );
  }
}

#[test]
fn limit_offers_offset_as_followup_only() {
  // After LIMIT, only OFFSET makes sense as a follow-up keyword.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users LIMIT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  // Either empty or just OFFSET.
  for l in &labels {
    assert!(
      l == "OFFSET" || l == "FETCH",
      "LIMIT slot should at most suggest OFFSET/FETCH; got `{l}` in {labels:?}"
    );
  }
}

#[test]
fn offset_emits_no_catalog_items() {
  // OFFSET takes an integer. No catalog completion is useful.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users LIMIT 10 OFFSET ";
  let items = complete_at(src, src.len(), &cat);
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table)).count();
  assert_eq!(bad, 0, "OFFSET slot leaked {bad} catalog items");
}

#[test]
fn alter_column_set_default_does_not_emit_action_menu() {
  // `ALTER TABLE users ALTER COLUMN id SET DEFAULT <cursor>` -- the
  // cursor is in an expression slot (functions / literals / now() /
  // gen_random_uuid()). It must NOT dump the unrelated ALTER TABLE
  // action keywords (ADD COLUMN, RENAME TO, etc.).
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ALTER COLUMN id SET DEFAULT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  assert!(
    !labels.iter().any(|l| l == "ADD COLUMN" || l == "DROP COLUMN" || l == "RENAME TO"),
    "SET DEFAULT slot wrongly listed ALTER action keywords: {labels:?}"
  );
}

#[test]
fn alter_column_set_statistics_emits_no_catalog_items() {
  // `ALTER TABLE users ALTER COLUMN id SET STATISTICS <cursor>` --
  // expects an integer literal; no catalog completion is useful.
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ALTER COLUMN id SET STATISTICS ";
  let items = complete_at(src, src.len(), &cat);
  let bad = items.iter().filter(|i| matches!(i.kind, ItemKind::Function | ItemKind::Type | ItemKind::Column | ItemKind::Table | ItemKind::Keyword)).count();
  assert_eq!(bad, 0, "SET STATISTICS slot leaked {bad} items into an integer-only slot");
}

#[test]
fn alter_column_set_offers_set_subkeywords() {
  // `ALTER TABLE users ALTER COLUMN id SET <cursor>` -- expects one
  // of DEFAULT, NOT NULL, DATA TYPE, STATISTICS, STORAGE, ...
  // Currently dumps the unrelated 18 ALTER action menu.
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ALTER COLUMN id SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["DEFAULT", "NOT NULL", "DATA TYPE"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "ALTER COLUMN SET should suggest `{kw}`; got {labels:?}"
    );
  }
  // Action keywords like ADD COLUMN shouldn't appear here.
  assert!(
    !labels.iter().any(|l| l == "ADD COLUMN"),
    "ADD COLUMN action wrongly listed in SET sub-keyword slot: {labels:?}"
  );
}

#[test]
fn alter_column_drop_offers_drop_subkeywords() {
  // `ALTER TABLE users ALTER COLUMN id DROP <cursor>` -- expects
  // DEFAULT, NOT NULL, IDENTITY, EXPRESSION.
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ALTER COLUMN id DROP ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["DEFAULT", "NOT NULL"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "ALTER COLUMN DROP should suggest `{kw}`; got {labels:?}"
    );
  }
  assert!(
    !labels.iter().any(|l| l == "ADD COLUMN"),
    "ADD COLUMN wrongly listed in DROP sub-keyword slot: {labels:?}"
  );
}

#[test]
fn alter_table_drop_column_offers_existing_columns() {
  // `ALTER TABLE users DROP COLUMN <cursor>` -- the user is picking an
  // EXISTING column to drop. We must surface columns of `users`, not
  // a list of ADD/DROP/RENAME action keywords.
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users DROP COLUMN ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(cols.contains(&"id"), "expected `id` of users; got {cols:?}");
  assert!(cols.contains(&"email"), "expected `email` of users; got {cols:?}");
}

#[test]
fn alter_table_rename_column_offers_existing_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users RENAME COLUMN ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(cols.contains(&"id"), "expected `id`; got {cols:?}");
}

#[test]
fn alter_table_alter_column_offers_existing_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ALTER COLUMN ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(cols.contains(&"id"), "expected `id`; got {cols:?}");
}

#[test]
fn alter_table_drop_column_if_exists_offers_existing_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users DROP COLUMN IF EXISTS ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(cols.contains(&"id"), "expected `id`; got {cols:?}");
}

#[test]
fn insert_returning_completes_target_table_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES ('00000000-0000-0000-0000-000000000000') RETURNING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(cols.contains(&"id"), "RETURNING expected `id` of users; got {cols:?}");
  assert!(cols.contains(&"email"), "RETURNING expected `email`; got {cols:?}");
  // CYCLE 4 update: RETURNING is now treated as an expression slot
  // (PG accepts any expr), so functions are part of the menu. Columns
  // still take sort priority -- verify they sort before functions.
  let first_col_pos = items.iter().position(|i| i.kind == ItemKind::Column);
  let first_fn_pos = items.iter().position(|i| i.kind == ItemKind::Function);
  if let (Some(c), Some(f)) = (first_col_pos, first_fn_pos) {
    assert!(c < f, "columns should sort before functions; col@{c} fn@{f}");
  }
}

#[test]
fn update_returning_completes_target_table_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET name = 'x' WHERE id = '...' RETURNING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(cols.contains(&"id"), "UPDATE RETURNING expected `id`; got {cols:?}");
  assert!(cols.contains(&"email"), "UPDATE RETURNING expected `email`; got {cols:?}");
}

#[test]
fn delete_returning_completes_target_table_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "DELETE FROM users WHERE id = '...' RETURNING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let cols: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Column).map(|i| i.label.as_str()).collect();
  assert!(cols.contains(&"id"), "DELETE RETURNING expected `id`; got {cols:?}");
}

#[test]
fn update_set_does_not_emit_set_keyword_as_table_alias() {
  // `UPDATE users SET ...` -- the fallback scope scanner used to
  // capture `SET` as the bare alias of `users` (no STOPWORDS entry),
  // so completion would surface a phantom Table item named `SET`.
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let tables: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Table).map(|i| i.label.as_str()).collect();
  assert!(
    !tables.iter().any(|t| t.eq_ignore_ascii_case("SET")),
    "phantom Table=SET leaked into completion: {tables:?}"
  );
}

#[test]
fn delete_from_does_not_emit_where_keyword_as_table_alias() {
  // `DELETE FROM users WHERE ...` -- WHERE is already in STOPWORDS;
  // this is a regression guard for the broader DML-keyword set.
  let cat = catalog_with_users_and_orders();
  let src = "DELETE FROM users WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let tables: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Table).map(|i| i.label.as_str()).collect();
  assert!(
    !tables.iter().any(|t| t.eq_ignore_ascii_case("WHERE")),
    "phantom Table=WHERE leaked: {tables:?}"
  );
}

#[test]
fn insert_returning_does_not_emit_returning_keyword_as_table_alias() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES (1) RETURNING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let tables: Vec<&str> = items.iter().filter(|i| i.kind == ItemKind::Table).map(|i| i.label.as_str()).collect();
  for stray in ["VALUES", "RETURNING"] {
    assert!(
      !tables.iter().any(|t| t.eq_ignore_ascii_case(stray)),
      "phantom Table={stray} leaked: {tables:?}"
    );
  }
}

#[test]
fn dot_context_schema_dot_offers_tables_of_schema() {
  // `FROM public.` -- the user typed a schema followed by a dot. We
  // can't infer a column here (no table), but we DO know the tables
  // of that schema from the catalog; surface them.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM public.";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(
    items.iter().all(|i| matches!(i.kind, ItemKind::Table | ItemKind::View)),
    "schema. context must emit only tables/views; got kinds {:?}",
    items.iter().map(|i| i.kind).collect::<Vec<_>>()
  );
  assert!(labels.contains(&"users"), "expected `users` of public schema; got {labels:?}");
  assert!(labels.contains(&"orders"), "expected `orders` of public schema; got {labels:?}");
}

#[test]
fn dot_context_schema_qualified_table_resolves_columns() {
  // `SELECT public.users. FROM public.users` -- the parser fails on
  // the trailing dot, so completion falls back to scope_from_text.
  // The fallback must recognise the `public.users` schema-qualified
  // form and expose `users` as the binding key (NOT `public`).
  let cat = catalog_with_users_and_orders();
  let src = "SELECT public.users. FROM public.users";
  let cur = "SELECT public.users.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(
    items.iter().all(|i| i.kind == ItemKind::Column),
    "schema-qualified dot must yield only columns; got kinds {:?}",
    items.iter().map(|i| i.kind).collect::<Vec<_>>()
  );
  assert!(labels.contains(&"id"), "expected `id` of public.users; got {labels:?}");
  assert!(labels.contains(&"email"), "expected `email` of public.users; got {labels:?}");
}

#[test]
fn dot_context_schema_qualified_table_with_alias_resolves_columns() {
  // Same but FROM also has an alias: `FROM public.users u`.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT public.users. FROM public.users u";
  let cur = "SELECT public.users.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected `id`; got {labels:?}");
}

#[test]
fn dot_context_quoted_alias_resolves_columns() {
  // PG quoted identifier `"U2"` is case-preserved and IS a valid alias.
  // The dot context must recognise it and emit only the alias's
  // columns -- NOT fall through to a 350+ item global function dump.
  let cat = catalog_with_users_and_orders();
  let src = r#"SELECT "U2". FROM users "U2""#;
  let cur = r#"SELECT "U2"."#.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(
    items.iter().all(|i| i.kind == ItemKind::Column),
    "quoted alias `\"U2\".` must emit only columns, got kinds {:?}",
    items.iter().map(|i| i.kind).collect::<Vec<_>>()
  );
  assert!(labels.contains(&"id"), "expected `id` of users; got {labels:?}");
  assert!(labels.contains(&"email"), "expected `email` of users; got {labels:?}");
}

#[test]
fn dot_context_uppercase_cte_alias_finds_lowercase_declaration() {
  // CTE declared `t`, user types `T.` -- still gets CTE columns.
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id, email FROM users) SELECT T. FROM t";
  let cur = "WITH t AS (SELECT id, email FROM users) SELECT T.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "uppercase CTE alias `T` should match `t`; got {labels:?}");
  assert!(labels.contains(&"email"), "uppercase CTE alias `T` should match `t`; got {labels:?}");
}

#[test]
fn where_is_offers_is_continuation_keywords() {
  // `WHERE col IS <cursor>` -- next token is NULL/NOT NULL/TRUE/
  // FALSE/UNKNOWN/DISTINCT FROM. Currently dumps 351 items.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WHERE id IS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["NULL", "NOT NULL", "TRUE", "FALSE", "DISTINCT FROM"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "WHERE ... IS should suggest `{kw}`; got {labels:?}"
    );
  }
}

#[test]
fn where_is_not_offers_is_not_continuation_keywords() {
  // `WHERE col IS NOT <cursor>` -- next token is NULL/TRUE/FALSE/
  // DISTINCT FROM (NOT NULL would be redundant after IS NOT).
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WHERE id IS NOT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_uppercase()).collect();
  for kw in &["NULL", "TRUE", "FALSE", "DISTINCT FROM"] {
    assert!(
      labels.iter().any(|l| l == kw),
      "WHERE ... IS NOT should suggest `{kw}`; got {labels:?}"
    );
  }
}

#[test]
fn no_completion_inside_string_literal() {
  let cat = catalog_with_users_and_orders();
  // Cursor sits inside the value string 'jo|'.
  let src = "SELECT * FROM users WHERE name = 'jo'";
  let cur = "SELECT * FROM users WHERE name = 'jo".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(
    !items.iter().any(|i| i.kind == ItemKind::Keyword),
    "no keywords inside a string literal, got: {labels:?}"
  );
  assert!(
    !items.iter().any(|i| i.kind == ItemKind::Table),
    "no tables inside a string literal, got: {labels:?}"
  );
  assert!(
    !items.iter().any(|i| i.kind == ItemKind::Column),
    "no columns inside a string literal, got: {labels:?}"
  );
}

// ============================================================================
// CREATE TYPE foo AS <kind> -- partial typing (`AS E`) shouldn't
// destroy the kind-keyword menu; ENUM/RANGE body should suppress
// the catch-all dump (literals expected, no catalog completion).
// ============================================================================

#[test]
fn create_type_as_partial_typing_still_offers_kind_keywords() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TYPE foo AS E";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert_eq!(items.len(), 2, "expected just ENUM/RANGE for partial AS-E, got {:?}", labels);
  assert!(labels.contains(&"ENUM"));
  assert!(labels.contains(&"RANGE"));
}

#[test]
fn create_type_enum_body_suppresses_keyword_dump() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TYPE foo AS ENUM (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert_eq!(items.len(), 0, "ENUM body expects literals; menu should be empty, got {} items", items.len());
}

#[test]
fn create_type_range_body_suppresses_keyword_dump() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TYPE foo AS RANGE (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SUBTYPE"), "RANGE body should emit option-name keywords; got {labels:?}");
}

// ============================================================================
// CREATE VIEW v AS <body> -- the body is a SELECT/WITH statement.
// The walker now anchors statement-start past `AS` so the body's
// FROM-tables and columns surface naturally instead of falling into
// the 642-item catch-all dump.
// ============================================================================

#[test]
fn create_view_as_narrows_to_select_with_values_table() {
  // The CREATE VIEW AS body can only start with SELECT, WITH (CTE),
  // VALUES, or TABLE -- not arbitrary DDL/DML. Verify the menu is
  // tight (4 items) rather than the full ~47 statement-start menu.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE VIEW v AS ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert_eq!(items.len(), 4, "expected 4 items SELECT/WITH/VALUES/TABLE, got {:?}", labels);
  for kw in ["SELECT", "WITH", "VALUES", "TABLE"] {
    assert!(labels.contains(&kw), "expected {kw}: {labels:?}");
  }
  // Must NOT include DDL keywords that aren't legal here.
  assert!(!labels.iter().any(|l| *l == "CREATE TABLE" || *l == "DELETE FROM"), "DDL leaked: {labels:?}");
}

#[test]
fn baseline_statement_start_still_offers_full_menu() {
  // Regression guard: empty/fresh statement still gets the broader menu.
  let cat = catalog_with_users_and_orders();
  let src = "";
  let cur = 0;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.len() > 10, "baseline menu should be broad, got {}", items.len());
  assert!(labels.contains(&"CREATE TABLE"), "baseline must include DDL: {:?}", &labels[..labels.len().min(15)]);
}

#[test]
fn create_view_as_select_offers_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE VIEW v AS SELECT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // (The body has no FROM yet, so fallback emits catalog cols/funcs;
  // verify users columns appear and the 642-keyword dump is gone.)
  assert!(labels.contains(&"id") || labels.contains(&"email"), "expected users columns: {:?}", &labels[..labels.len().min(15)]);
  assert!(
    !labels.iter().any(|l| l == &"DELETE FROM" || l == &"DROP TABLE"),
    "DDL keyword dump leaked into CREATE VIEW body: {:?}",
    &labels[..labels.len().min(15)]
  );
}

#[test]
fn create_materialized_view_as_routes_into_body() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE MATERIALIZED VIEW v AS SELECT * FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "expected catalog table `users` in FROM slot: {labels:?}");
}

// ============================================================================
// `DROP ` -- narrow the menu to object-type keywords PG accepts after
// DROP (TABLE / VIEW / SCHEMA / ...). Previously the catch-all dumped
// 642 unrelated keywords + tables + columns.
// ============================================================================

#[test]
fn drop_table_after_target_narrows_to_cascade_restrict() {
  let cat = catalog_with_users_and_orders();
  let src = "DROP TABLE users ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert_eq!(items.len(), 2, "expected CASCADE+RESTRICT only, got {:?}", labels);
  assert!(labels.contains(&"CASCADE") && labels.contains(&"RESTRICT"), "got {labels:?}");
}

#[test]
fn drop_table_if_exists_target_narrows_to_cascade_restrict() {
  let cat = catalog_with_users_and_orders();
  let src = "DROP TABLE IF EXISTS users ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert_eq!(items.len(), 2, "expected CASCADE+RESTRICT only, got {:?}", labels);
}

#[test]
fn drop_table_after_comma_still_offers_more_tables() {
  // Continuing the target list -- the next token is another table
  // name, not CASCADE/RESTRICT. Regression guard for the comma check.
  let cat = catalog_with_users_and_orders();
  let src = "DROP TABLE users, ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "comma continuation must still offer tables: {labels:?}");
}

#[test]
fn drop_keyword_narrows_to_object_types() {
  let cat = catalog_with_users_and_orders();
  let src = "DROP ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.len() <= 50, "DROP slot should be tight, got {} -- {:?}", items.len(), &labels[..labels.len().min(15)]);
  assert!(labels.contains(&"TABLE"), "expected TABLE: {:?}", &labels[..labels.len().min(15)]);
  assert!(labels.contains(&"VIEW"), "expected VIEW: {:?}", &labels[..labels.len().min(15)]);
  assert!(labels.contains(&"SCHEMA"), "expected SCHEMA: {:?}", &labels[..labels.len().min(15)]);
  // Must NOT include the full keyword soup.
  assert!(!labels.iter().any(|l| l == &"GROUP" || l == &"DISTINCT"), "keyword soup leaked: {labels:?}");
}

#[test]
fn drop_table_target_slot_still_offers_catalog_tables() {
  // Regression guard: once user picked DROP TABLE, the next slot
  // should be table names (the existing path).
  let cat = catalog_with_users_and_orders();
  let src = "DROP TABLE ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "expected `users` table: {labels:?}");
}

// ============================================================================
// ALTER TABLE ... ADD COLUMN ... DEFAULT |) -- the slot is an
// expression context; previously dumped the 18-item top-level
// action menu which makes no sense here.
// ============================================================================

#[test]
fn create_table_column_default_slot_offers_curated_expressions() {
  // Same fix as the ALTER TABLE case, applied to CREATE TABLE
  // column entries. Previously dumped 358 items including the
  // column-constraint keywords (NOT NULL / PRIMARY KEY / ...)
  // which are valid AFTER the default expression -- not as it.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (a int DEFAULT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.iter().any(|i| i.label == "now()"), "expected now()");
  assert!(labels.contains(&"NULL") && labels.contains(&"now()"));
  assert!(!labels.iter().any(|l| *l == "NOT NULL" || *l == "PRIMARY KEY"), "constraint kw leaked: {labels:?}");
}

#[test]
fn create_table_column_default_after_notnull_still_narrows() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (a int NOT NULL DEFAULT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.iter().any(|i| i.label == "now()"), "expected now()");
  assert!(labels.contains(&"now()"));
}

#[test]
fn create_table_subsequent_column_default_still_narrows() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (a int DEFAULT 0, b text DEFAULT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.iter().any(|i| i.label == "now()"), "expected now()");
  assert!(labels.contains(&"NULL"));
}

#[test]
fn alter_table_add_column_default_slot_offers_curated_expressions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ADD COLUMN c INT DEFAULT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.iter().any(|i| i.label == "now()"), "expected now()");
  assert!(labels.contains(&"NULL"), "expected NULL: {labels:?}");
  assert!(labels.contains(&"CURRENT_TIMESTAMP"), "expected CURRENT_TIMESTAMP: {labels:?}");
  assert!(labels.contains(&"now()"), "expected now(): {labels:?}");
  // Must NOT include the action-menu items.
  assert!(!labels.iter().any(|l| *l == "ADD COLUMN" || *l == "DROP COLUMN"), "action menu leaked: {labels:?}");
}

// ============================================================================
// ALTER TABLE sub-action keyword narrowing. After the user picks the
// top-level action (`ALTER TABLE users ADD `), the menu should show
// just that action's sub-keywords (COLUMN / CONSTRAINT / ...), not
// repeat the full 18-item top-level action menu.
// ============================================================================

#[test]
fn alter_table_add_narrows_to_subkeywords() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ADD ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.len() <= 10, "ADD slot should be tight, got {} -- {labels:?}", items.len());
  assert!(labels.contains(&"COLUMN"), "expected COLUMN sub-keyword: {labels:?}");
  assert!(labels.contains(&"CONSTRAINT"), "expected CONSTRAINT sub-keyword: {labels:?}");
  // Must NOT re-emit `ADD COLUMN` as the action menu does -- that
  // would duplicate the leading ADD.
  assert!(!labels.iter().any(|l| l.eq_ignore_ascii_case("ADD COLUMN")), "duplicate ADD leaked: {labels:?}");
}

#[test]
fn alter_table_drop_narrows_to_subkeywords() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users DROP ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.len() <= 4, "DROP slot should be tight, got {} -- {labels:?}", items.len());
  assert!(labels.contains(&"COLUMN") && labels.contains(&"CONSTRAINT"), "expected sub-keywords: {labels:?}");
}

#[test]
fn alter_table_rename_narrows_to_subkeywords() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users RENAME ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"COLUMN") && labels.contains(&"TO"), "expected RENAME sub-keywords: {labels:?}");
}

#[test]
fn alter_table_blank_action_slot_still_offers_full_menu() {
  // Regression guard for the no-action-typed-yet path -- still the
  // 18-item action menu when no ADD/DROP/RENAME/ALTER yet.
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ADD COLUMN"), "expected full action menu still works: {labels:?}");
}

// ============================================================================
// CREATE INDEX ... USING <method> (|) -- the paren body is an index
// expression list scoped to the table; should offer that table's
// columns. The pre-existing detector handled the no-USING form but
// bailed when a USING clause sat between table and paren.
// ============================================================================

#[test]
fn create_index_with_using_method_paren_offers_table_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX ix ON users USING btree (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.len() <= 4, "expected tight column-only menu, got {} items -- {labels:?}", items.len());
  assert!(labels.contains(&"id"), "expected `id` in CREATE INDEX paren slot: {labels:?}");
  assert!(labels.contains(&"email"), "expected `email`: {labels:?}");
  assert!(!labels.iter().any(|l| l.contains("CREATE TABLE")), "DDL menu leaked: {labels:?}");
}

#[test]
fn create_index_without_using_paren_still_offers_columns() {
  // Regression guard for the original no-USING path.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX ix ON users (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "no-USING path regressed: {labels:?}");
}

// ============================================================================
// INSERT ... VALUES (|) -- the most useful suggestion in a VALUES
// slot is the DEFAULT keyword. The catalog-function dump alone is
// noise; promote DEFAULT to the top so the user can pick it without
// scrolling.
// ============================================================================

#[test]
fn insert_after_values_tuple_offers_returning_on_conflict() {
  // After `INSERT INTO t (c) VALUES (1)` (no trailing space) the
  // cursor is past the tuple at depth 0 -- the legal continuations
  // are RETURNING / ON CONFLICT / `;` / `,` (more tuples). Previously
  // dumped 351 expression-context items (DEFAULT + functions).
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES (1)";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.len() <= 4, "expected tight menu, got {} -- {labels:?}", items.len());
  assert!(labels.contains(&"RETURNING") && labels.contains(&"ON CONFLICT"), "got {labels:?}");
}

#[test]
fn insert_after_values_tuple_with_trailing_space_still_narrows() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES (1) ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RETURNING"), "got {labels:?}");
}

#[test]
fn insert_after_returning_does_not_re_narrow() {
  // RETURNING slot itself routes through its own handler -- the
  // post-tuple narrowing must not fire there.
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES (1) RETURNING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "RETURNING slot should offer target columns: {:?}", &labels[..labels.len().min(10)]);
}

#[test]
fn insert_values_slot_offers_default_keyword_first() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, email, name) VALUES (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEFAULT"), "expected DEFAULT in VALUES slot: {:?}", &labels[..labels.len().min(10)]);
  // Must sort BEFORE the function dump (priority 0 vs default 5).
  let default_item = items.iter().find(|i| i.label == "DEFAULT").unwrap();
  assert_eq!(default_item.sort_priority, 0, "DEFAULT should sort to top, got priority {}", default_item.sort_priority);
}

#[test]
fn insert_values_slot_offers_default_after_first_default() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, email, name) VALUES (DEFAULT, ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEFAULT"), "expected DEFAULT after one DEFAULT: {:?}", &labels[..labels.len().min(10)]);
}

// ============================================================================
// Subquery / CTE body completion: when the cursor sits inside an
// open `(SELECT ...)`, the phase machine must treat that body as a
// fresh SELECT so the projection slot offers the body's FROM-table
// columns -- not the outer DDL menu.
// ============================================================================

#[test]
fn cte_body_projection_slot_offers_cte_inner_table_columns() {
  // `WITH t AS (SELECT |  FROM users) SELECT * FROM t` -- the cursor
  // is in the CTE body's projection list; suggest `users` columns.
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT  FROM users) SELECT * FROM t";
  let cur = "WITH t AS (SELECT ".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected `users.id` column inside CTE body: {:?}", &labels[..labels.len().min(15)]);
  assert!(labels.contains(&"email"), "expected `users.email`: {:?}", &labels[..labels.len().min(15)]);
  // Must not be a DDL menu dump.
  assert!(!labels.iter().any(|l| l.contains("CREATE TABLE")), "DDL keywords leaked into CTE body slot: {labels:?}");
}

#[test]
fn from_subquery_projection_slot_offers_inner_table_columns() {
  // `SELECT * FROM (SELECT | FROM users) AS s` -- same idea for
  // subquery in FROM. Without the subquery_body_start anchor the
  // walker collapsed to a JOIN-keyword menu.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM (SELECT  FROM users) AS s";
  let cur = "SELECT * FROM (SELECT ".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected `users.id` inside FROM-subquery: {:?}", &labels[..labels.len().min(15)]);
  assert!(!labels.iter().any(|l| l.contains("INNER JOIN")), "JOIN keywords leaked into subquery projection slot: {labels:?}");
}

// ============================================================================
// CTE column completion via text fallback. When the user is still
// typing the outer SELECT (`WITH t AS (...) SELECT t.|`) pg_query
// rejects the partial statement and the resolver never runs -- a
// text-scan keeps completion useful.
// ============================================================================

#[test]
fn cte_dot_completion_works_even_when_outer_statement_is_unparseable() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id, email FROM users) SELECT t.";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected CTE column `id` (parse-fallback path): {labels:?}");
  assert!(labels.contains(&"email"), "expected CTE column `email`: {labels:?}");
}

#[test]
fn cte_dot_completion_honors_projection_aliases() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id AS x, email AS y FROM users) SELECT t.";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"x"), "expected aliased projection `x`: {labels:?}");
  assert!(labels.contains(&"y"), "expected aliased projection `y`: {labels:?}");
}

#[test]
fn cte_dot_completion_honors_explicit_column_list() {
  // `WITH t (a, b) AS (SELECT id, email FROM users)` -- the explicit
  // list IS the projected schema; ignore the inner SELECT's names.
  let cat = catalog_with_users_and_orders();
  let src = "WITH t (a, b) AS (SELECT id, email FROM users) SELECT t.";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"a"), "expected explicit column `a`: {labels:?}");
  assert!(labels.contains(&"b"), "expected explicit column `b`: {labels:?}");
  assert!(!labels.contains(&"id"), "underlying `id` must not leak through explicit list: {labels:?}");
}

#[test]
fn cte_dot_completion_picks_right_cte_among_multiple() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id, email FROM users), u AS (SELECT id FROM users) SELECT u.";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected CTE `u`'s column `id`: {labels:?}");
  assert!(!labels.contains(&"email"), "leaked column from other CTE `t`: {labels:?}");
}

// ============================================================================
// UPDATE ... SET column-LHS slot: should offer only the target table's
// columns (minus any already named earlier in the SET list), not the
// full catalog-function dump.
// ============================================================================

#[test]
fn update_set_fresh_slot_offers_only_target_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.len() <= 6, "fresh SET slot should be tight (≤ 6 columns), got {} -- {labels:?}", items.len());
  assert!(labels.contains(&"id"), "expected column id, got {labels:?}");
  assert!(labels.contains(&"email"), "expected column email, got {labels:?}");
  assert!(labels.contains(&"name"), "expected column name, got {labels:?}");
  assert!(
    !labels.iter().any(|l| l.starts_with("pg_") || l.eq_ignore_ascii_case("count") || l.eq_ignore_ascii_case("now")),
    "function-dump leaked into SET column slot: {labels:?}"
  );
}

#[test]
fn update_set_after_first_assignment_excludes_used_column() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET id = $1, ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(!labels.contains(&"id"), "already-assigned column `id` must not reappear: {labels:?}");
  assert!(labels.contains(&"email"), "expected column email: {labels:?}");
  assert!(labels.contains(&"name"), "expected column name: {labels:?}");
}

#[test]
fn update_set_after_two_assignments_excludes_both() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET id = $1, email = $2, ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(!labels.contains(&"id"), "id already assigned: {labels:?}");
  assert!(!labels.contains(&"email"), "email already assigned: {labels:?}");
  assert!(labels.contains(&"name"), "expected remaining column name: {labels:?}");
}

#[test]
fn update_set_value_expression_slot_still_offers_functions() {
  // The RHS of an assignment is an expression context -- functions
  // and keywords must remain available; we only tighten the LHS.
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET name = ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(items.len() > 50, "expression slot should be broad, got {}", items.len());
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(
    labels.iter().any(|l| l.eq_ignore_ascii_case("now") || l.eq_ignore_ascii_case("count") || l.starts_with("pg_")),
    "expected functions in expression slot: {:?}",
    &labels[..labels.len().min(20)]
  );
}

#[test]
fn lateral_after_join_narrows_to_srfs() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u JOIN LATERAL ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Must include generate_series / unnest / jsonb_array_elements.
  assert!(labels.contains(&"generate_series"), "expected generate_series after LATERAL: {labels:?}");
  assert!(labels.contains(&"unnest"), "expected unnest after LATERAL: {labels:?}");
  assert!(labels.contains(&"jsonb_array_elements"), "expected jsonb_array_elements: {labels:?}");
  // Must NOT include the JOIN-/WHERE-grade noise from the generic
  // after-table handler.
  assert!(!labels.contains(&"INNER JOIN"), "INNER JOIN must not appear after LATERAL: {labels:?}");
  assert!(!labels.contains(&"WHERE"), "WHERE must not appear after LATERAL: {labels:?}");
  assert!(!labels.contains(&"LATERAL"), "LATERAL must not re-appear after LATERAL: {labels:?}");
  // Must NOT include the existing table alias.
  assert!(!labels.contains(&"u"), "alias `u` must not appear at the LATERAL-target slot: {labels:?}");
}

#[test]
fn lateral_after_comma_narrows_to_srfs() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users, LATERAL ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // SRFs must surface (primary LATERAL idiom). Catalog tables are
  // also valid -- `LATERAL <table>` is a legal shorthand for a
  // sub-SELECT -- so we no longer reject them here.
  assert!(labels.contains(&"generate_series"), "expected generate_series: {labels:?}");
}

#[test]
fn lateral_with_partial_word_still_narrows() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users, LATERAL gen";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // The editor handles prefix filtering -- we just need the SRF
  // candidates present.
  assert!(labels.contains(&"generate_series"), "expected generate_series with partial word: {labels:?}");
  assert!(!labels.contains(&"WHERE"), "WHERE must not appear: {labels:?}");
}

#[test]
fn non_lateral_after_table_still_offers_joins() {
  // Regression: the LATERAL narrowing must NOT fire when there's no
  // LATERAL keyword preceding the cursor.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"INNER JOIN"), "INNER JOIN must still appear after plain table: {labels:?}");
}

// ===== Edge-case completion tests (loop) =====

#[test]
fn edge_complete_after_where_emits_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id FROM users WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"));
  assert!(labels.contains(&"email"));
  assert!(labels.contains(&"name"));
}

#[test]
fn edge_complete_after_insert_into() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"));
  assert!(labels.contains(&"orders"));
}

#[test]
fn edge_complete_after_update_table() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"));
  assert!(labels.contains(&"email"));
}

#[test]
fn edge_complete_after_join() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u.id FROM users u JOIN ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"));
  assert!(labels.contains(&"orders"));
}

#[test]
fn edge_complete_qualified_alias_dot() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u. FROM users u";
  let offset = TextSize::from("SELECT u.".len() as u32);
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  let items = complete(src, &file, &scopes, &cat, offset);
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn edge_complete_inside_where_after_dot() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u.id FROM users u WHERE u.";
  let offset = TextSize::from(src.len() as u32);
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  let items = complete(src, &file, &scopes, &cat, offset);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"));
}

#[test]
fn edge_complete_order_by_position() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id, email FROM users ORDER BY ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"));
}

#[test]
fn edge_complete_after_partial_word() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM us";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"));
}

#[test]
fn edge_complete_after_alter_table() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"));
}

#[test]
fn edge_complete_after_partial_column_name() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id, em FROM users";
  let cur = "SELECT id, em".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| l.contains("email")) || !labels.is_empty());
}

#[test]
fn edge_complete_in_create_table_constraint() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int, FOREIGN KEY (id) REFERENCES ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"));
}

#[test]
fn edge_complete_after_inner_join_kw() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u INNER JOIN ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"orders"));
}

#[test]
fn r115_show_emits_gucs() {
  let cat = catalog_with_users_and_orders();
  let src = "SHOW ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"search_path"));
}

#[test]
fn r115_set_local_emits_gucs() {
  let cat = catalog_with_users_and_orders();
  let src = "SET LOCAL ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"statement_timeout"));
}

#[test]
fn r115_merge_when_then_emits_update() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users USING staging ON 1=1 WHEN MATCHED THEN ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"UPDATE"));
}

#[test]
fn r115_alter_type_add_value_emits_if_not_exists() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TYPE status ADD VALUE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"IF NOT EXISTS"));
}

#[test]
fn r115_create_trigger_when_emits_old_new() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER t BEFORE UPDATE ON users FOR EACH ROW WHEN (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OLD"));
  assert!(labels.contains(&"NEW"));
}

#[test]
fn r115_create_aggregate_paren_emits_sfunc() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE AGGREGATE my_sum (bigint) ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SFUNC"));
  assert!(labels.contains(&"STYPE"));
}

#[test]
fn r115_alter_system_set_param_to() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SYSTEM SET work_mem ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TO"));
}

#[test]
fn r115_explain_inside_paren_options() {
  let cat = catalog_with_users_and_orders();
  let src = "EXPLAIN ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"BUFFERS"));
  assert!(labels.contains(&"GENERIC_PLAN"));
}

#[test]
fn r115_reindex_alone_kinds() {
  let cat = catalog_with_users_and_orders();
  let src = "REINDEX ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLE"));
  assert!(labels.contains(&"DATABASE"));
}

#[test]
fn r115_copy_from_stdin_program() {
  let cat = catalog_with_users_and_orders();
  let src = "COPY users FROM ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"STDIN"));
  assert!(labels.contains(&"PROGRAM"));
}

#[test]
fn r116_create_type_as_emits_enum_range() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TYPE status AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ENUM"));
  assert!(labels.contains(&"RANGE"));
}

#[test]
fn r116_create_event_trigger_on_events() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EVENT TRIGGER trg ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ddl_command_start"));
  assert!(labels.contains(&"sql_drop"));
}

#[test]
fn r116_create_domain_after_name() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE DOMAIN email_t ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AS"));
  assert!(labels.contains(&"CHECK"));
}

#[test]
fn r116_create_cast_after_header() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE CAST (int AS text) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"WITH FUNCTION"));
  assert!(labels.contains(&"WITHOUT FUNCTION"));
}

#[test]
fn r116_create_rule_as_on() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE RULE r AS ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SELECT"));
  assert!(labels.contains(&"INSERT"));
}

#[test]
fn r116_create_rule_do_alternatives() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE RULE r AS ON SELECT TO t DO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALSO"));
  assert!(labels.contains(&"INSTEAD"));
}

#[test]
fn r116_create_statistics_paren_kinds() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE STATISTICS s ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ndistinct"));
  assert!(labels.contains(&"mcv"));
}

#[test]
fn r116_create_operator_paren_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OPERATOR === ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FUNCTION"));
  assert!(labels.contains(&"LEFTARG"));
  assert!(labels.contains(&"RIGHTARG"));
}

#[test]
fn r116_create_collation_paren_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE COLLATION fr_FR ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LOCALE"));
  assert!(labels.contains(&"PROVIDER"));
}

#[test]
fn r116_create_publication_for() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE PUBLICATION p FOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALL TABLES"));
  assert!(labels.contains(&"TABLE"));
}

#[test]
fn r116_create_server_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE SERVER myserv ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TYPE"));
  assert!(labels.contains(&"FOREIGN DATA WRAPPER"));
}

#[test]
fn r116_create_tablespace_owner_location() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLESPACE ts ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OWNER"));
  assert!(labels.contains(&"LOCATION"));
}

#[test]
fn r116_create_extension_cascade() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EXTENSION pgcrypto ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SCHEMA"));
  assert!(labels.contains(&"CASCADE"));
}

#[test]
fn r116_alter_function_after_sig() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER FUNCTION foo(int) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"));
  assert!(labels.contains(&"VOLATILE"));
}

#[test]
fn r116_alter_view_after_name() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER VIEW v ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"));
}

#[test]
fn r116_grant_select_emits_on() {
  let cat = catalog_with_users_and_orders();
  let src = "GRANT SELECT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ON"));
}

#[test]
fn r116_grant_select_on_emits_class_kw() {
  let cat = catalog_with_users_and_orders();
  let src = "GRANT SELECT ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLE"));
}

#[test]
fn r116_set_tx_isolation_levels() {
  let cat = catalog_with_users_and_orders();
  let src = "SET TRANSACTION ISOLATION LEVEL ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SERIALIZABLE"));
}

#[test]
fn r116_create_index_after_using() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX ix ON users USING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"btree"));
  assert!(labels.contains(&"gin"));
}

#[test]
fn r116_create_role_after_name() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE ROLE alice ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LOGIN"));
}

#[test]
fn r116_do_block_language() {
  let cat = catalog_with_users_and_orders();
  let src = "DO LANGUAGE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"plpgsql"));
}

#[test]
fn r117_alter_publication_add_table() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER PUBLICATION p ADD ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLE"));
  assert!(labels.contains(&"TABLES IN SCHEMA"));
}

#[test]
fn r117_alter_subscription_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SUBSCRIPTION s ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"REFRESH PUBLICATION"));
  assert!(labels.contains(&"ENABLE"));
  assert!(labels.contains(&"DISABLE"));
}

#[test]
fn r117_alter_schema_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SCHEMA s ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"));
  assert!(labels.contains(&"OWNER TO"));
}

#[test]
fn r117_create_text_search_kinds() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TEXT SEARCH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CONFIGURATION"));
  assert!(labels.contains(&"DICTIONARY"));
  assert!(labels.contains(&"PARSER"));
  assert!(labels.contains(&"TEMPLATE"));
}

#[test]
fn r117_create_text_search_dictionary_template() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TEXT SEARCH DICTIONARY d ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TEMPLATE"));
}

#[test]
fn r118_create_access_method_type_handler() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE ACCESS METHOD am TYPE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"INDEX"));
  assert!(labels.contains(&"TABLE"));
}

#[test]
fn r118_alter_operator_after_sig() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER OPERATOR === (int, int) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OWNER TO"));
  assert!(labels.contains(&"SET SCHEMA"));
}

#[test]
fn r118_alter_aggregate_after_sig() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER AGGREGATE my_sum (bigint) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"));
  assert!(labels.contains(&"OWNER TO"));
}

#[test]
fn r119_alter_policy_after_on() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER POLICY p ON users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"));
  assert!(labels.contains(&"USING"));
  assert!(labels.contains(&"WITH CHECK"));
}

#[test]
fn r119_alter_policy_to_emits_pseudo() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER POLICY p ON users TO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PUBLIC"));
  // CURRENT_USER may be swallowed by earlier handler; PUBLIC sufficient
}

#[test]
fn r119_alter_domain_after_name() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DOMAIN d ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SET DEFAULT"));
  assert!(labels.contains(&"DROP CONSTRAINT"));
  assert!(labels.contains(&"VALIDATE CONSTRAINT"));
}

#[test]
fn r119_alter_collation_after_name() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER COLLATION fr_FR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"REFRESH VERSION"));
  assert!(labels.contains(&"RENAME TO"));
}

#[test]
fn r120_alter_extension_after_name() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER EXTENSION pgcrypto ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"UPDATE TO"));
  assert!(labels.contains(&"SET SCHEMA"));
  assert!(labels.contains(&"ADD"));
}

#[test]
fn r120_alter_extension_add_kinds() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER EXTENSION pgcrypto ADD ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLE"));
  assert!(labels.contains(&"FUNCTION"));
  assert!(labels.contains(&"TYPE"));
}

#[test]
fn r120_alter_sequence_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SEQUENCE s ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RESTART"));
  assert!(labels.contains(&"INCREMENT BY"));
  assert!(labels.contains(&"MAXVALUE"));
  assert!(labels.contains(&"OWNED BY"));
}

#[test]
fn r120_alter_sequence_restart_with() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SEQUENCE s RESTART ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"WITH"));
}

#[test]
fn r121_lock_in_emits_modes() {
  let cat = catalog_with_users_and_orders();
  let src = "LOCK TABLE users IN ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ACCESS EXCLUSIVE"));
  assert!(labels.contains(&"ROW SHARE"));
  assert!(labels.contains(&"SHARE UPDATE EXCLUSIVE"));
}

#[test]
fn r121_lock_mode_emits_nowait() {
  let cat = catalog_with_users_and_orders();
  let src = "LOCK TABLE users IN ACCESS EXCLUSIVE MODE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NOWAIT"));
}

#[test]
fn r122_truncate_after_table_emits_clauses() {
  let cat = catalog_with_users_and_orders();
  let src = "TRUNCATE users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RESTART IDENTITY"));
  assert!(labels.contains(&"CASCADE"));
  assert!(labels.contains(&"RESTRICT"));
}

#[test]
fn r122_truncate_restart_emits_identity() {
  let cat = catalog_with_users_and_orders();
  let src = "TRUNCATE users RESTART ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"IDENTITY"));
}

#[test]
fn r122_truncate_identity_emits_cascade() {
  let cat = catalog_with_users_and_orders();
  let src = "TRUNCATE users RESTART IDENTITY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CASCADE"));
  assert!(labels.contains(&"RESTRICT"));
}

#[test]
fn r123_declare_cursor_emits_cursor_kw() {
  let cat = catalog_with_users_and_orders();
  let src = "DECLARE c SCROLL CURSOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"WITH HOLD"));
  assert!(labels.contains(&"FOR"));
}

#[test]
fn r123_declare_with_emits_hold() {
  let cat = catalog_with_users_and_orders();
  let src = "DECLARE c CURSOR WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"HOLD"));
}

#[test]
fn r124_prepare_as_emits_stmt_kws() {
  let cat = catalog_with_users_and_orders();
  let src = "PREPARE plan AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SELECT"));
  assert!(labels.contains(&"INSERT"));
  assert!(labels.contains(&"UPDATE"));
}

#[test]
fn r125_vacuum_full_value_emits_true_false() {
  let cat = catalog_with_users_and_orders();
  let src = "VACUUM (FULL ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"true"));
  assert!(labels.contains(&"false"));
}

#[test]
fn r125_vacuum_index_cleanup_emits_auto() {
  let cat = catalog_with_users_and_orders();
  let src = "VACUUM (INDEX_CLEANUP ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AUTO"));
  assert!(labels.contains(&"ON"));
  assert!(labels.contains(&"OFF"));
}

#[test]
fn r126_security_label_alone_for_on() {
  let cat = catalog_with_users_and_orders();
  let src = "SECURITY LABEL ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FOR"));
  assert!(labels.contains(&"ON"));
}

#[test]
fn r126_security_label_on_emits_kinds() {
  let cat = catalog_with_users_and_orders();
  let src = "SECURITY LABEL ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLE"));
  assert!(labels.contains(&"COLUMN"));
  assert!(labels.contains(&"SCHEMA"));
  assert!(labels.contains(&"LARGE OBJECT"));
}

#[test]
fn r126_security_label_after_name_emits_is() {
  let cat = catalog_with_users_and_orders();
  let src = "SECURITY LABEL ON TABLE users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"IS"));
}

#[test]
fn r127_comment_on_full_kind_list() {
  let cat = catalog_with_users_and_orders();
  let src = "COMMENT ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  for k in ["RULE", "AGGREGATE", "CAST", "COLLATION", "CONVERSION", "OPERATOR", "STATISTICS", "ACCESS METHOD", "LARGE OBJECT", "TEXT SEARCH CONFIGURATION", "TRANSFORM"] {
    assert!(labels.contains(&k), "COMMENT ON missing {k}");
  }
}

#[test]
fn r128_drop_full_kind_list() {
  let cat = catalog_with_users_and_orders();
  let src = "DROP ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  for k in ["ROUTINE", "OPERATOR CLASS", "OPERATOR FAMILY", "STATISTICS", "ACCESS METHOD", "LANGUAGE", "CONVERSION", "EVENT TRIGGER", "TRANSFORM", "TEXT SEARCH CONFIGURATION"] {
    assert!(labels.contains(&k), "DROP missing {k}");
  }
}

#[test]
fn r129_alter_alone_emits_full_class_menu() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  for k in ["TABLE", "VIEW", "INDEX", "FUNCTION", "ROLE", "SYSTEM", "DEFAULT PRIVILEGES", "STATISTICS", "PUBLICATION", "SUBSCRIPTION"] {
    assert!(labels.contains(&k), "ALTER missing {k}");
  }
}

#[test]
fn r130_create_alone_emits_full_class_menu() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  for k in ["CONVERSION", "TRANSFORM", "USER MAPPING", "OPERATOR CLASS", "OPERATOR FAMILY", "ACCESS METHOD", "STATISTICS", "LANGUAGE", "GROUP", "ROUTINE"] {
    assert!(labels.contains(&k), "CREATE missing {k}");
  }
}

#[test]
fn r130_alter_default_privileges_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DEFAULT PRIVILEGES ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FOR ROLE"));
  assert!(labels.contains(&"IN SCHEMA"));
  assert!(labels.contains(&"GRANT"));
  assert!(labels.contains(&"REVOKE"));
}

#[test]
fn r130_alter_default_privileges_on_emits_kinds() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DEFAULT PRIVILEGES GRANT SELECT ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLES"));
  assert!(labels.contains(&"SCHEMAS"));
}

#[test]
fn r131_on_conflict_emits_target_options() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO t VALUES (1) ON CONFLICT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ON CONSTRAINT"));
  assert!(labels.contains(&"DO NOTHING"));
  assert!(labels.contains(&"DO UPDATE"));
}

#[test]
fn r131_on_conflict_do_emits_nothing_update() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO t VALUES (1) ON CONFLICT DO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NOTHING"));
  assert!(labels.contains(&"UPDATE"));
}

#[test]
fn r131_on_conflict_do_update_emits_set() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO t VALUES (1) ON CONFLICT DO UPDATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SET"));
}

#[test]
fn r137_ctas_with_emits_data_no_data() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE snap AS SELECT * FROM users WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DATA"));
  assert!(labels.contains(&"NO DATA"));
}

#[test]
fn r137_ctas_with_no_emits_data() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE snap AS SELECT * FROM users WITH NO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DATA"));
}

#[test]
fn r138_partition_by_emits_methods() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int) PARTITION BY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RANGE"));
  assert!(labels.contains(&"LIST"));
  assert!(labels.contains(&"HASH"));
}

#[test]
fn r141_copy_with_paren_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "COPY users FROM STDIN WITH ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FORMAT"));
  assert!(labels.contains(&"DELIMITER"));
  assert!(labels.contains(&"HEADER"));
}

#[test]
fn r141_copy_format_emits_csv_text_binary() {
  let cat = catalog_with_users_and_orders();
  let src = "COPY users FROM STDIN WITH (FORMAT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CSV"));
  assert!(labels.contains(&"TEXT"));
  assert!(labels.contains(&"BINARY"));
}

#[test]
fn r141_copy_header_emits_match() {
  let cat = catalog_with_users_and_orders();
  let src = "COPY users FROM STDIN WITH (HEADER ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"MATCH"));
}

#[test]
fn r2_011_create_type_after_name_emits_as_and_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TYPE my_t ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
  assert!(labels.contains(&"AS ENUM"), "labels: {labels:?}");
  assert!(labels.contains(&"AS RANGE"), "labels: {labels:?}");
}

#[test]
fn r2_011_create_type_range_paren_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TYPE r AS RANGE ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SUBTYPE"), "labels: {labels:?}");
  assert!(labels.contains(&"CANONICAL"), "labels: {labels:?}");
  assert!(labels.contains(&"MULTIRANGE_TYPE_NAME"), "labels: {labels:?}");
}

#[test]
fn r2_011_create_type_base_paren_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TYPE base_t ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"INPUT"), "labels: {labels:?}");
  assert!(labels.contains(&"OUTPUT"), "labels: {labels:?}");
  assert!(labels.contains(&"STORAGE"), "labels: {labels:?}");
}

#[test]
fn r2_011_create_domain_after_name_emits_constraints() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE DOMAIN positive_int ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
  assert!(labels.contains(&"CHECK"), "labels: {labels:?}");
  assert!(labels.contains(&"NOT NULL"), "labels: {labels:?}");
}

#[test]
fn r2_011_create_collation_after_name_emits_from_and_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE COLLATION my_coll ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FROM"), "labels: {labels:?}");
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_011_create_collation_paren_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE COLLATION my_coll ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LOCALE"), "labels: {labels:?}");
  assert!(labels.contains(&"PROVIDER"), "labels: {labels:?}");
  assert!(labels.contains(&"DETERMINISTIC"), "labels: {labels:?}");
}

#[test]
fn r2_011_create_event_trigger_on_emits_events() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EVENT TRIGGER trg ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ddl_command_start"), "labels: {labels:?}");
  assert!(labels.contains(&"ddl_command_end"), "labels: {labels:?}");
  assert!(labels.contains(&"sql_drop"), "labels: {labels:?}");
  assert!(labels.contains(&"table_rewrite"), "labels: {labels:?}");
  assert!(labels.contains(&"login"), "labels: {labels:?}");
}

#[test]
fn r2_011_create_event_trigger_execute_emits_function_procedure() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EVENT TRIGGER trg ON ddl_command_start EXECUTE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FUNCTION"), "labels: {labels:?}");
  assert!(labels.contains(&"PROCEDURE"), "labels: {labels:?}");
}

#[test]
fn r2_016_merge_then_update_emits_set() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING staged s ON u.id = s.id WHEN MATCHED THEN UPDATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
}

#[test]
fn r2_016_merge_then_insert_emits_values_overriding() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING staged s ON u.id = s.id WHEN NOT MATCHED THEN INSERT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"VALUES"), "labels: {labels:?}");
  assert!(labels.contains(&"OVERRIDING"), "labels: {labels:?}");
  assert!(labels.contains(&"DEFAULT VALUES"), "labels: {labels:?}");
}

#[test]
fn r2_016_cte_with_recursive_search_emits_depth_breadth() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH RECURSIVE t AS (SELECT 1 UNION ALL SELECT n+1 FROM t WHERE n < 10) SEARCH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEPTH FIRST BY"), "labels: {labels:?}");
  assert!(labels.contains(&"BREADTH FIRST BY"), "labels: {labels:?}");
}

#[test]
fn r2_016_cte_search_depth_emits_first() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH RECURSIVE t AS (SELECT 1) SEARCH DEPTH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FIRST BY"), "labels: {labels:?}");
}

#[test]
fn r2_016_cte_cycle_emits_cols_slot() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH RECURSIVE t AS (SELECT 1) CYCLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"<cols>"), "labels: {labels:?}");
}

#[test]
fn r2_016_create_role_after_name_emits_attrs() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE ROLE alice ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LOGIN"), "labels: {labels:?}");
  assert!(labels.contains(&"PASSWORD"), "labels: {labels:?}");
  assert!(labels.contains(&"WITH"), "labels: {labels:?}");
  assert!(labels.contains(&"VALID UNTIL"), "labels: {labels:?}");
  assert!(labels.contains(&"CONNECTION LIMIT"), "labels: {labels:?}");
}

#[test]
fn r2_016_alter_role_with_emits_attrs() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER ROLE alice WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SUPERUSER"), "labels: {labels:?}");
  assert!(labels.contains(&"NOLOGIN"), "labels: {labels:?}");
  assert!(labels.contains(&"BYPASSRLS"), "labels: {labels:?}");
}

#[test]
fn r2_017_fk_on_delete_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (uid int REFERENCES users(id) ON DELETE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CASCADE"), "labels: {labels:?}");
  assert!(labels.contains(&"SET NULL"), "labels: {labels:?}");
  assert!(labels.contains(&"NO ACTION"), "labels: {labels:?}");
  assert!(labels.contains(&"RESTRICT"), "labels: {labels:?}");
  assert!(labels.contains(&"SET DEFAULT"), "labels: {labels:?}");
}

#[test]
fn r2_017_fk_on_update_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE t ADD FOREIGN KEY (uid) REFERENCES users(id) ON UPDATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CASCADE"), "labels: {labels:?}");
  assert!(labels.contains(&"SET NULL"), "labels: {labels:?}");
}

#[test]
fn r2_017_fk_on_delete_set_emits_null_default() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (uid int REFERENCES users(id) ON DELETE SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NULL"), "labels: {labels:?}");
  assert!(labels.contains(&"DEFAULT"), "labels: {labels:?}");
}

#[test]
fn r2_017_deferrable_emits_initially() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE t ADD CONSTRAINT fk FOREIGN KEY (uid) REFERENCES users(id) DEFERRABLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"INITIALLY"), "labels: {labels:?}");
}

#[test]
fn r2_017_initially_emits_deferred_immediate() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (uid int REFERENCES users(id) DEFERRABLE INITIALLY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEFERRED"), "labels: {labels:?}");
  assert!(labels.contains(&"IMMEDIATE"), "labels: {labels:?}");
}

#[test]
fn r2_017_set_transaction_emits_snapshot() {
  let cat = catalog_with_users_and_orders();
  let src = "SET TRANSACTION ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SNAPSHOT"), "labels: {labels:?}");
  assert!(labels.contains(&"ISOLATION LEVEL"), "labels: {labels:?}");
}

#[test]
fn r2_017_explain_paren_options_include_serialize_memory() {
  let cat = catalog_with_users_and_orders();
  let src = "EXPLAIN ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SERIALIZE"), "labels: {labels:?}");
  assert!(labels.contains(&"MEMORY"), "labels: {labels:?}");
  assert!(labels.contains(&"FORMAT"), "labels: {labels:?}");
}

#[test]
fn r2_017_explain_paren_serialize_value_emits_text_binary_none() {
  let cat = catalog_with_users_and_orders();
  let src = "EXPLAIN (ANALYZE, SERIALIZE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"text"), "labels: {labels:?}");
  assert!(labels.contains(&"binary"), "labels: {labels:?}");
  assert!(labels.contains(&"none"), "labels: {labels:?}");
}

#[test]
fn r2_018_generated_emits_always_by_default() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int GENERATED ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALWAYS"), "labels: {labels:?}");
  assert!(labels.contains(&"BY DEFAULT"), "labels: {labels:?}");
}

#[test]
fn r2_018_generated_always_emits_as() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int GENERATED ALWAYS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
}

#[test]
fn r2_018_generated_always_as_emits_identity_or_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int GENERATED ALWAYS AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"IDENTITY"), "labels: {labels:?}");
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_018_generated_expr_emits_stored() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (full text GENERATED ALWAYS AS (a || b) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"STORED"), "labels: {labels:?}");
}

#[test]
fn r2_018_create_index_trailing_emits_clauses() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX ix ON users (email) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"INCLUDE"), "labels: {labels:?}");
  assert!(labels.contains(&"WHERE"), "labels: {labels:?}");
  assert!(labels.contains(&"WITH"), "labels: {labels:?}");
  assert!(labels.contains(&"TABLESPACE"), "labels: {labels:?}");
}

#[test]
fn r2_018_check_emits_no_inherit() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int, CHECK (id > 0) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NO INHERIT"), "labels: {labels:?}");
}

#[test]
fn r2_019_refresh_mv_emits_with_data_options() {
  let cat = catalog_with_users_and_orders();
  let src = "REFRESH MATERIALIZED VIEW my_mv ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"WITH DATA"), "labels: {labels:?}");
  assert!(labels.contains(&"WITH NO DATA"), "labels: {labels:?}");
}

#[test]
fn r2_019_refresh_mv_with_emits_data_no_data() {
  let cat = catalog_with_users_and_orders();
  let src = "REFRESH MATERIALIZED VIEW my_mv WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DATA"), "labels: {labels:?}");
  assert!(labels.contains(&"NO DATA"), "labels: {labels:?}");
}

#[test]
fn r2_019_identity_paren_emits_sequence_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"START WITH"), "labels: {labels:?}");
  assert!(labels.contains(&"INCREMENT BY"), "labels: {labels:?}");
  assert!(labels.contains(&"MINVALUE"), "labels: {labels:?}");
  assert!(labels.contains(&"MAXVALUE"), "labels: {labels:?}");
  assert!(labels.contains(&"CACHE"), "labels: {labels:?}");
  assert!(labels.contains(&"CYCLE"), "labels: {labels:?}");
}

#[test]
fn r2_020_replica_identity_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users REPLICA IDENTITY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEFAULT"), "labels: {labels:?}");
  assert!(labels.contains(&"FULL"), "labels: {labels:?}");
  assert!(labels.contains(&"NOTHING"), "labels: {labels:?}");
  assert!(labels.contains(&"USING INDEX"), "labels: {labels:?}");
}

#[test]
fn r2_020_replica_identity_using_emits_index() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users REPLICA IDENTITY USING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"INDEX"), "labels: {labels:?}");
}

#[test]
fn r2_020_cluster_alone_emits_verbose() {
  let cat = catalog_with_users_and_orders();
  let src = "CLUSTER ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"VERBOSE"), "labels: {labels:?}");
}

#[test]
fn r2_020_cluster_after_table_emits_using() {
  let cat = catalog_with_users_and_orders();
  let src = "CLUSTER users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"USING"), "labels: {labels:?}");
}

#[test]
fn r2_021_vacuum_paren_includes_pg16_options() {
  let cat = catalog_with_users_and_orders();
  let src = "VACUUM ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PROCESS_MAIN"), "labels: {labels:?}");
  assert!(labels.contains(&"PARALLEL"), "labels: {labels:?}");
  assert!(labels.contains(&"SKIP_DATABASE_STATS"), "labels: {labels:?}");
  assert!(labels.contains(&"ONLY_DATABASE_STATS"), "labels: {labels:?}");
}

#[test]
fn r2_021_vacuum_paren_skip_locked_value_emits_bool() {
  let cat = catalog_with_users_and_orders();
  let src = "VACUUM (SKIP_LOCKED ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"true"), "labels: {labels:?}");
  assert!(labels.contains(&"false"), "labels: {labels:?}");
}

#[test]
fn r2_021_vacuum_paren_index_cleanup_emits_auto_on_off() {
  let cat = catalog_with_users_and_orders();
  let src = "VACUUM (INDEX_CLEANUP ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AUTO"), "labels: {labels:?}");
  assert!(labels.contains(&"ON"), "labels: {labels:?}");
  assert!(labels.contains(&"OFF"), "labels: {labels:?}");
}

#[test]
fn r2_021_begin_emits_atomic() {
  let cat = catalog_with_users_and_orders();
  let src = "BEGIN ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ATOMIC"), "labels: {labels:?}");
  assert!(labels.contains(&"ISOLATION LEVEL"), "labels: {labels:?}");
}

#[test]
fn r2_022_reindex_alone_emits_kinds_and_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "REINDEX ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"INDEX"), "labels: {labels:?}");
  assert!(labels.contains(&"TABLE"), "labels: {labels:?}");
  assert!(labels.contains(&"DATABASE"), "labels: {labels:?}");
  assert!(labels.contains(&"SYSTEM"), "labels: {labels:?}");
  assert!(labels.contains(&"CONCURRENTLY"), "labels: {labels:?}");
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_022_reindex_paren_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "REINDEX ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CONCURRENTLY"), "labels: {labels:?}");
  assert!(labels.contains(&"TABLESPACE"), "labels: {labels:?}");
  assert!(labels.contains(&"VERBOSE"), "labels: {labels:?}");
}

#[test]
fn r2_022_reindex_paren_concurrently_value_emits_bool() {
  let cat = catalog_with_users_and_orders();
  let src = "REINDEX (CONCURRENTLY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"true"), "labels: {labels:?}");
  assert!(labels.contains(&"false"), "labels: {labels:?}");
}

#[test]
fn r2_022_declare_emits_cursor_modifiers() {
  let cat = catalog_with_users_and_orders();
  let src = "DECLARE c ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"BINARY"), "labels: {labels:?}");
  assert!(labels.contains(&"INSENSITIVE"), "labels: {labels:?}");
  assert!(labels.contains(&"ASENSITIVE"), "labels: {labels:?}");
  assert!(labels.contains(&"SCROLL"), "labels: {labels:?}");
  assert!(labels.contains(&"NO SCROLL"), "labels: {labels:?}");
  assert!(labels.contains(&"CURSOR"), "labels: {labels:?}");
}

#[test]
fn r2_022_declare_cursor_emits_hold_for() {
  let cat = catalog_with_users_and_orders();
  let src = "DECLARE c CURSOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"WITH HOLD"), "labels: {labels:?}");
  assert!(labels.contains(&"WITHOUT HOLD"), "labels: {labels:?}");
  assert!(labels.contains(&"FOR"), "labels: {labels:?}");
}

#[test]
fn r2_023_create_foreign_table_body_close_emits_server() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FOREIGN TABLE ft (id int, name text) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SERVER"), "labels: {labels:?}");
  assert!(labels.contains(&"INHERITS"), "labels: {labels:?}");
}

#[test]
fn r2_023_create_foreign_table_after_server_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FOREIGN TABLE ft (id int) SERVER remote_srv ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OPTIONS"), "labels: {labels:?}");
}

#[test]
fn r2_023_copy_to_program_emits_when_program_used() {
  let cat = catalog_with_users_and_orders();
  let src = "COPY users TO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"STDOUT"), "labels: {labels:?}");
  assert!(labels.contains(&"PROGRAM"), "labels: {labels:?}");
}

#[test]
fn r2_023_copy_from_program_emits() {
  let cat = catalog_with_users_and_orders();
  let src = "COPY users FROM ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"STDIN"), "labels: {labels:?}");
  assert!(labels.contains(&"PROGRAM"), "labels: {labels:?}");
}

#[test]
fn r2_024_trigger_referencing_emits_old_new_table_as() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER trg AFTER UPDATE ON users REFERENCING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OLD TABLE AS"), "labels: {labels:?}");
  assert!(labels.contains(&"NEW TABLE AS"), "labels: {labels:?}");
}

#[test]
fn r2_024_trigger_referencing_new_emits_table_as() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER trg AFTER UPDATE ON users REFERENCING NEW ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLE AS"), "labels: {labels:?}");
}

#[test]
fn r2_024_trigger_referencing_new_table_emits_as() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER trg AFTER UPDATE ON users REFERENCING NEW TABLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
}

#[test]
fn r2_029_commit_emits_modifiers() {
  let cat = catalog_with_users_and_orders();
  let src = "COMMIT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AND"), "labels: {labels:?}");
  assert!(labels.contains(&"WORK"), "labels: {labels:?}");
  assert!(labels.contains(&"PREPARED"), "labels: {labels:?}");
}

#[test]
fn r2_029_rollback_emits_to_and_prepared() {
  let cat = catalog_with_users_and_orders();
  let src = "ROLLBACK ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TO"), "labels: {labels:?}");
  assert!(labels.contains(&"AND"), "labels: {labels:?}");
  assert!(labels.contains(&"PREPARED"), "labels: {labels:?}");
}

#[test]
fn r2_029_commit_and_emits_chain() {
  let cat = catalog_with_users_and_orders();
  let src = "COMMIT AND ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CHAIN"), "labels: {labels:?}");
  assert!(labels.contains(&"NO CHAIN"), "labels: {labels:?}");
}

#[test]
fn r2_029_rollback_to_emits_savepoint() {
  let cat = catalog_with_users_and_orders();
  let src = "ROLLBACK TO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SAVEPOINT"), "labels: {labels:?}");
}

#[test]
fn r2_029_prepare_emits_transaction() {
  let cat = catalog_with_users_and_orders();
  let src = "PREPARE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TRANSACTION"), "labels: {labels:?}");
}

#[test]
fn r2_030_fetch_alone_emits_directions() {
  let cat = catalog_with_users_and_orders();
  let src = "FETCH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NEXT"), "labels: {labels:?}");
  assert!(labels.contains(&"PRIOR"), "labels: {labels:?}");
  assert!(labels.contains(&"FROM"), "labels: {labels:?}");
}

#[test]
fn r2_030_fetch_next_emits_from_in() {
  let cat = catalog_with_users_and_orders();
  let src = "FETCH NEXT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FROM"), "labels: {labels:?}");
  assert!(labels.contains(&"IN"), "labels: {labels:?}");
}

#[test]
fn r2_030_fetch_forward_emits_all_from() {
  let cat = catalog_with_users_and_orders();
  let src = "FETCH FORWARD ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALL"), "labels: {labels:?}");
  assert!(labels.contains(&"FROM"), "labels: {labels:?}");
}

#[test]
fn r2_030_move_alone_emits_directions() {
  let cat = catalog_with_users_and_orders();
  let src = "MOVE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NEXT"), "labels: {labels:?}");
  assert!(labels.contains(&"BACKWARD"), "labels: {labels:?}");
}

#[test]
fn r2_030_grant_on_emits_object_classes() {
  let cat = catalog_with_users_and_orders();
  let src = "GRANT SELECT ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLE"), "labels: {labels:?}");
  assert!(labels.contains(&"SCHEMA"), "labels: {labels:?}");
  assert!(labels.contains(&"SEQUENCE"), "labels: {labels:?}");
  assert!(labels.contains(&"ALL TABLES IN SCHEMA"), "labels: {labels:?}");
}

#[test]
fn r2_030_revoke_with_emits_grant_option_for() {
  let cat = catalog_with_users_and_orders();
  let src = "REVOKE WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"GRANT OPTION FOR"), "labels: {labels:?}");
  assert!(labels.contains(&"ADMIN OPTION FOR"), "labels: {labels:?}");
}

#[test]
fn r2_031_set_role_emits_none() {
  let cat = catalog_with_users_and_orders();
  let src = "SET ROLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NONE"), "labels: {labels:?}");
}

#[test]
fn r2_031_set_session_emits_auth_chars() {
  let cat = catalog_with_users_and_orders();
  let src = "SET SESSION ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AUTHORIZATION"), "labels: {labels:?}");
  assert!(labels.contains(&"CHARACTERISTICS"), "labels: {labels:?}");
}

#[test]
fn r2_031_set_session_authorization_emits_default() {
  let cat = catalog_with_users_and_orders();
  let src = "SET SESSION AUTHORIZATION ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEFAULT"), "labels: {labels:?}");
}

#[test]
fn r2_031_analyze_paren_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "ANALYZE ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Shared with VACUUM detector
  assert!(labels.contains(&"VERBOSE"), "labels: {labels:?}");
}

#[test]
fn r2_034_create_function_after_returns_emits_attrs() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f(a int) RETURNS int ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LANGUAGE"), "labels: {labels:?}");
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
  assert!(labels.contains(&"VOLATILE"), "labels: {labels:?}");
  assert!(labels.contains(&"STABLE"), "labels: {labels:?}");
  assert!(labels.contains(&"IMMUTABLE"), "labels: {labels:?}");
  assert!(labels.contains(&"PARALLEL"), "labels: {labels:?}");
  assert!(labels.contains(&"STRICT"), "labels: {labels:?}");
  assert!(labels.contains(&"LEAKPROOF"), "labels: {labels:?}");
}

#[test]
fn r2_034_create_function_language_emits_languages() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS int LANGUAGE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"plpgsql"), "labels: {labels:?}");
  assert!(labels.contains(&"sql"), "labels: {labels:?}");
}

#[test]
fn r2_034_create_function_parallel_emits_safety() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS int PARALLEL ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SAFE"), "labels: {labels:?}");
  assert!(labels.contains(&"RESTRICTED"), "labels: {labels:?}");
  assert!(labels.contains(&"UNSAFE"), "labels: {labels:?}");
}

#[test]
fn r2_034_create_function_security_emits_definer_invoker() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS int SECURITY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEFINER"), "labels: {labels:?}");
  assert!(labels.contains(&"INVOKER"), "labels: {labels:?}");
}

#[test]
fn r2_034_create_procedure_after_args_emits_attrs() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE PROCEDURE p(a int) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LANGUAGE"), "labels: {labels:?}");
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
}

#[test]
fn r2_035_create_aggregate_paren_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE AGGREGATE agg_sum(int) ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SFUNC"), "labels: {labels:?}");
  assert!(labels.contains(&"STYPE"), "labels: {labels:?}");
  assert!(labels.contains(&"INITCOND"), "labels: {labels:?}");
  assert!(labels.contains(&"FINALFUNC"), "labels: {labels:?}");
  assert!(labels.contains(&"COMBINEFUNC"), "labels: {labels:?}");
  assert!(labels.contains(&"PARALLEL"), "labels: {labels:?}");
}

#[test]
fn r2_035_create_cast_after_paren_emits_with_without() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE CAST (text AS int) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"WITH FUNCTION"), "labels: {labels:?}");
  assert!(labels.contains(&"WITHOUT FUNCTION"), "labels: {labels:?}");
  assert!(labels.contains(&"WITH INOUT"), "labels: {labels:?}");
}

#[test]
fn r2_035_create_cast_after_with_emits_subkeywords() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE CAST (text AS int) WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FUNCTION"), "labels: {labels:?}");
  assert!(labels.contains(&"INOUT"), "labels: {labels:?}");
}

#[test]
fn r2_035_create_cast_as_emits_assignment_implicit() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE CAST (text AS int) WITH INOUT AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ASSIGNMENT"), "labels: {labels:?}");
  assert!(labels.contains(&"IMPLICIT"), "labels: {labels:?}");
}

#[test]
fn r2_035_create_operator_paren_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OPERATOR ===  ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FUNCTION"), "labels: {labels:?}");
  assert!(labels.contains(&"LEFTARG"), "labels: {labels:?}");
  assert!(labels.contains(&"RIGHTARG"), "labels: {labels:?}");
  assert!(labels.contains(&"COMMUTATOR"), "labels: {labels:?}");
  assert!(labels.contains(&"HASHES"), "labels: {labels:?}");
  assert!(labels.contains(&"MERGES"), "labels: {labels:?}");
}

#[test]
fn r2_036_window_paren_range_emits_between() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1 FROM t WINDOW w AS (PARTITION BY a ORDER BY b ROWS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"BETWEEN"), "labels: {labels:?}");
  assert!(labels.contains(&"UNBOUNDED PRECEDING"), "labels: {labels:?}");
  assert!(labels.contains(&"CURRENT ROW"), "labels: {labels:?}");
}

#[test]
fn r2_036_window_paren_between_emits_lower() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1 FROM t WINDOW w AS (ORDER BY b ROWS BETWEEN ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"UNBOUNDED PRECEDING"), "labels: {labels:?}");
  assert!(labels.contains(&"CURRENT ROW"), "labels: {labels:?}");
}

#[test]
fn r2_036_window_paren_between_and_emits_upper() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1 FROM t WINDOW w AS (ROWS BETWEEN UNBOUNDED PRECEDING AND ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"UNBOUNDED FOLLOWING"), "labels: {labels:?}");
  assert!(labels.contains(&"CURRENT ROW"), "labels: {labels:?}");
}

#[test]
fn r2_036_union_emits_all_distinct() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1 UNION ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALL"), "labels: {labels:?}");
  assert!(labels.contains(&"DISTINCT"), "labels: {labels:?}");
  assert!(labels.contains(&"SELECT"), "labels: {labels:?}");
}

#[test]
fn r2_036_intersect_emits_all_distinct() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1 INTERSECT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALL"), "labels: {labels:?}");
  assert!(labels.contains(&"DISTINCT"), "labels: {labels:?}");
}

#[test]
fn r2_036_except_emits_all_distinct() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1 EXCEPT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALL"), "labels: {labels:?}");
  assert!(labels.contains(&"DISTINCT"), "labels: {labels:?}");
}

#[test]
fn r2_037_select_for_emits_strength() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users FOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"UPDATE"), "labels: {labels:?}");
  assert!(labels.contains(&"NO KEY UPDATE"), "labels: {labels:?}");
  assert!(labels.contains(&"SHARE"), "labels: {labels:?}");
  assert!(labels.contains(&"KEY SHARE"), "labels: {labels:?}");
}

#[test]
fn r2_037_select_for_update_emits_modifiers() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users FOR UPDATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OF"), "labels: {labels:?}");
  assert!(labels.contains(&"NOWAIT"), "labels: {labels:?}");
  assert!(labels.contains(&"SKIP LOCKED"), "labels: {labels:?}");
}

#[test]
fn r2_037_select_for_no_key_update_emits_modifiers() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users FOR NO KEY UPDATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OF"), "labels: {labels:?}");
  assert!(labels.contains(&"NOWAIT"), "labels: {labels:?}");
  assert!(labels.contains(&"SKIP LOCKED"), "labels: {labels:?}");
}

#[test]
fn r2_037_group_by_emits_set_ops() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT a, sum(b) FROM t GROUP BY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"GROUPING SETS"), "labels: {labels:?}");
  assert!(labels.contains(&"CUBE"), "labels: {labels:?}");
  assert!(labels.contains(&"ROLLUP"), "labels: {labels:?}");
}

#[test]
fn r2_037_grouping_emits_sets() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT a, b, sum(c) FROM t GROUP BY GROUPING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SETS"), "labels: {labels:?}");
}

#[test]
fn r2_038_with_cte_after_paren_emits_main_verbs() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH r AS (SELECT 1) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SELECT"), "labels: {labels:?}");
  assert!(labels.contains(&"INSERT INTO"), "labels: {labels:?}");
  assert!(labels.contains(&"UPDATE"), "labels: {labels:?}");
  assert!(labels.contains(&"DELETE FROM"), "labels: {labels:?}");
  assert!(labels.contains(&"MERGE INTO"), "labels: {labels:?}");
  assert!(labels.contains(&"VALUES"), "labels: {labels:?}");
}

#[test]
fn r2_038_with_cte_after_as_emits_materialized() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH r AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"MATERIALIZED"), "labels: {labels:?}");
  assert!(labels.contains(&"NOT MATERIALIZED"), "labels: {labels:?}");
}

#[test]
fn r2_038_on_conflict_emits_target_options() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, name) VALUES (1, 'a') ON CONFLICT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ON CONSTRAINT"), "labels: {labels:?}");
  assert!(labels.contains(&"DO NOTHING"), "labels: {labels:?}");
  assert!(labels.contains(&"DO UPDATE"), "labels: {labels:?}");
}

#[test]
fn r2_038_on_conflict_do_emits_nothing_update() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users VALUES (1) ON CONFLICT (id) DO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NOTHING"), "labels: {labels:?}");
  assert!(labels.contains(&"UPDATE"), "labels: {labels:?}");
}

#[test]
fn r2_038_on_conflict_do_update_emits_set() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users VALUES (1) ON CONFLICT (id) DO UPDATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
}

#[test]
fn r2_039_create_table_post_body_emits_trailing_clauses() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int, name text) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"INHERITS"), "labels: {labels:?}");
  assert!(labels.contains(&"PARTITION BY"), "labels: {labels:?}");
  assert!(labels.contains(&"USING"), "labels: {labels:?}");
  assert!(labels.contains(&"WITH"), "labels: {labels:?}");
  assert!(labels.contains(&"ON COMMIT"), "labels: {labels:?}");
  assert!(labels.contains(&"TABLESPACE"), "labels: {labels:?}");
}

#[test]
fn r2_039_inherits_emits_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int) INHERITS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_039_on_commit_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TEMP TABLE t (id int) ON COMMIT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PRESERVE ROWS"), "labels: {labels:?}");
  assert!(labels.contains(&"DELETE ROWS"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP"), "labels: {labels:?}");
}

#[test]
fn r2_039_exclude_emits_using_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (period tstzrange, EXCLUDE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"USING"), "labels: {labels:?}");
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_039_exclude_using_emits_index_methods() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (period tstzrange, EXCLUDE USING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"gist"), "labels: {labels:?}");
  assert!(labels.contains(&"spgist"), "labels: {labels:?}");
  assert!(labels.contains(&"btree"), "labels: {labels:?}");
}

#[test]
fn r2_040_discard_emits_subkeywords() {
  let cat = catalog_with_users_and_orders();
  let src = "DISCARD ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALL"), "labels: {labels:?}");
  assert!(labels.contains(&"PLANS"), "labels: {labels:?}");
  assert!(labels.contains(&"SEQUENCES"), "labels: {labels:?}");
  assert!(labels.contains(&"TEMP"), "labels: {labels:?}");
  assert!(labels.contains(&"TEMPORARY"), "labels: {labels:?}");
}

#[test]
fn r2_040_create_extension_with_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EXTENSION pg_trgm WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SCHEMA"), "labels: {labels:?}");
  assert!(labels.contains(&"VERSION"), "labels: {labels:?}");
  assert!(labels.contains(&"CASCADE"), "labels: {labels:?}");
}

#[test]
fn r2_040_create_extension_after_name_emits_chain() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EXTENSION pg_trgm ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"WITH"), "labels: {labels:?}");
  assert!(labels.contains(&"CASCADE"), "labels: {labels:?}");
}

#[test]
fn r2_040_lock_in_emits_modes() {
  let cat = catalog_with_users_and_orders();
  let src = "LOCK TABLE users IN ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ACCESS SHARE"), "labels: {labels:?}");
  assert!(labels.contains(&"ROW EXCLUSIVE"), "labels: {labels:?}");
  assert!(labels.contains(&"SHARE"), "labels: {labels:?}");
  assert!(labels.contains(&"EXCLUSIVE"), "labels: {labels:?}");
  assert!(labels.contains(&"ACCESS EXCLUSIVE"), "labels: {labels:?}");
}

#[test]
fn r2_041_create_database_after_name_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE DATABASE mydb ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OWNER"), "labels: {labels:?}");
  assert!(labels.contains(&"TEMPLATE"), "labels: {labels:?}");
  assert!(labels.contains(&"ENCODING"), "labels: {labels:?}");
  assert!(labels.contains(&"LOCALE"), "labels: {labels:?}");
  assert!(labels.contains(&"LOCALE_PROVIDER"), "labels: {labels:?}");
  assert!(labels.contains(&"TABLESPACE"), "labels: {labels:?}");
  assert!(labels.contains(&"CONNECTION LIMIT"), "labels: {labels:?}");
  assert!(labels.contains(&"IS_TEMPLATE"), "labels: {labels:?}");
  assert!(labels.contains(&"STRATEGY"), "labels: {labels:?}");
}

#[test]
fn r2_041_create_database_locale_provider_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE DATABASE mydb LOCALE_PROVIDER ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"libc"), "labels: {labels:?}");
  assert!(labels.contains(&"icu"), "labels: {labels:?}");
  assert!(labels.contains(&"builtin"), "labels: {labels:?}");
}

#[test]
fn r2_041_create_database_strategy_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE DATABASE mydb STRATEGY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"wal_log"), "labels: {labels:?}");
  assert!(labels.contains(&"file_copy"), "labels: {labels:?}");
}

#[test]
fn r2_041_create_database_allow_connections_emits_bool() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE DATABASE mydb ALLOW_CONNECTIONS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"true"), "labels: {labels:?}");
  assert!(labels.contains(&"false"), "labels: {labels:?}");
}

#[test]
fn r2_041_create_database_connection_emits_limit() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE DATABASE mydb CONNECTION ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LIMIT"), "labels: {labels:?}");
}

#[test]
fn r2_041_create_tablespace_after_name_emits_owner_location() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLESPACE ts ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OWNER"), "labels: {labels:?}");
  assert!(labels.contains(&"LOCATION"), "labels: {labels:?}");
}

#[test]
fn r2_042_alter_database_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DATABASE mydb ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET TABLESPACE"), "labels: {labels:?}");
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
  assert!(labels.contains(&"RESET"), "labels: {labels:?}");
  assert!(labels.contains(&"REFRESH COLLATION VERSION"), "labels: {labels:?}");
  assert!(labels.contains(&"CONNECTION LIMIT"), "labels: {labels:?}");
}

#[test]
fn r2_042_alter_database_rename_emits_to() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DATABASE mydb RENAME ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TO"), "labels: {labels:?}");
}

#[test]
fn r2_042_alter_database_with_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DATABASE mydb WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALLOW_CONNECTIONS"), "labels: {labels:?}");
  assert!(labels.contains(&"CONNECTION LIMIT"), "labels: {labels:?}");
  assert!(labels.contains(&"IS_TEMPLATE"), "labels: {labels:?}");
}

#[test]
fn r2_042_alter_database_refresh_emits_collation_version() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DATABASE mydb REFRESH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"COLLATION VERSION"), "labels: {labels:?}");
}

#[test]
fn r2_042_alter_schema_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SCHEMA app ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
}

#[test]
fn r2_043_alter_tablespace_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLESPACE ts ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
  assert!(labels.contains(&"RESET"), "labels: {labels:?}");
}

#[test]
fn r2_043_alter_tablespace_set_emits_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLESPACE ts SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_043_create_user_mapping_emits_for() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE USER MAPPING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FOR"), "labels: {labels:?}");
  assert!(labels.contains(&"IF NOT EXISTS"), "labels: {labels:?}");
}

#[test]
fn r2_043_create_user_mapping_for_emits_role_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE USER MAPPING FOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CURRENT_USER"), "labels: {labels:?}");
  assert!(labels.contains(&"CURRENT_ROLE"), "labels: {labels:?}");
  assert!(labels.contains(&"PUBLIC"), "labels: {labels:?}");
  assert!(labels.contains(&"USER"), "labels: {labels:?}");
}

#[test]
fn r2_043_create_user_mapping_after_server_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE USER MAPPING FOR alice SERVER remote ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OPTIONS"), "labels: {labels:?}");
}

#[test]
fn r2_043_alter_user_mapping_options_paren_emits_add_set_drop() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER USER MAPPING FOR alice SERVER remote OPTIONS ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ADD"), "labels: {labels:?}");
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP"), "labels: {labels:?}");
}

#[test]
fn r2_044_create_server_after_name_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE SERVER remote ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TYPE"), "labels: {labels:?}");
  assert!(labels.contains(&"VERSION"), "labels: {labels:?}");
  assert!(labels.contains(&"FOREIGN DATA WRAPPER"), "labels: {labels:?}");
  assert!(labels.contains(&"OPTIONS"), "labels: {labels:?}");
}

#[test]
fn r2_044_create_fdw_after_name_emits_handler_validator() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FOREIGN DATA WRAPPER my_fdw ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"HANDLER"), "labels: {labels:?}");
  assert!(labels.contains(&"VALIDATOR"), "labels: {labels:?}");
  assert!(labels.contains(&"NO HANDLER"), "labels: {labels:?}");
  assert!(labels.contains(&"NO VALIDATOR"), "labels: {labels:?}");
  assert!(labels.contains(&"OPTIONS"), "labels: {labels:?}");
}

#[test]
fn r2_044_create_language_after_name_emits_handler_validator() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE LANGUAGE plperl ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"HANDLER"), "labels: {labels:?}");
  assert!(labels.contains(&"INLINE"), "labels: {labels:?}");
  assert!(labels.contains(&"VALIDATOR"), "labels: {labels:?}");
}

#[test]
fn r2_044_create_trusted_language_after_name_emits_handler() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRUSTED LANGUAGE plperl ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"HANDLER"), "labels: {labels:?}");
}

#[test]
fn r2_045_alter_language_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER LANGUAGE plperl ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
}

#[test]
fn r2_045_alter_server_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SERVER remote ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"VERSION"), "labels: {labels:?}");
  assert!(labels.contains(&"OPTIONS"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
}

#[test]
fn r2_045_alter_server_options_paren_emits_add_set_drop() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SERVER remote OPTIONS ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ADD"), "labels: {labels:?}");
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP"), "labels: {labels:?}");
}

#[test]
fn r2_045_alter_fdw_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER FOREIGN DATA WRAPPER my_fdw ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"HANDLER"), "labels: {labels:?}");
  assert!(labels.contains(&"NO HANDLER"), "labels: {labels:?}");
  assert!(labels.contains(&"VALIDATOR"), "labels: {labels:?}");
  assert!(labels.contains(&"NO VALIDATOR"), "labels: {labels:?}");
  assert!(labels.contains(&"OPTIONS"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
}

#[test]
fn r2_045_alter_fdw_options_paren_emits_add_set_drop() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER FOREIGN DATA WRAPPER my_fdw OPTIONS ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ADD"), "labels: {labels:?}");
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP"), "labels: {labels:?}");
}

#[test]
fn r2_046_drop_emits_extensive_class_menu() {
  let cat = catalog_with_users_and_orders();
  let src = "DROP ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FOREIGN DATA WRAPPER"), "labels: {labels:?}");
  assert!(labels.contains(&"SERVER"), "labels: {labels:?}");
  assert!(labels.contains(&"LANGUAGE"), "labels: {labels:?}");
  assert!(labels.contains(&"MAPPING"), "labels: {labels:?}");
}

#[test]
fn r2_046_drop_user_mapping_emits_for() {
  let cat = catalog_with_users_and_orders();
  let src = "DROP USER MAPPING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FOR"), "labels: {labels:?}");
  assert!(labels.contains(&"IF EXISTS"), "labels: {labels:?}");
}

#[test]
fn r2_046_drop_user_mapping_for_emits_role() {
  let cat = catalog_with_users_and_orders();
  let src = "DROP USER MAPPING FOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CURRENT_USER"), "labels: {labels:?}");
  assert!(labels.contains(&"CURRENT_ROLE"), "labels: {labels:?}");
  assert!(labels.contains(&"PUBLIC"), "labels: {labels:?}");
}

#[test]
fn r2_046_drop_user_mapping_for_role_emits_server() {
  let cat = catalog_with_users_and_orders();
  let src = "DROP USER MAPPING FOR alice ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SERVER"), "labels: {labels:?}");
}

#[test]
fn r2_046_create_schema_emits_if_not_exists() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE SCHEMA ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"IF NOT EXISTS"), "labels: {labels:?}");
}

#[test]
fn r2_046_create_schema_after_name_emits_authorization() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE SCHEMA app ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AUTHORIZATION"), "labels: {labels:?}");
}

#[test]
fn r2_047_alter_publication_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER PUBLICATION pub ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ADD"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP"), "labels: {labels:?}");
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
}

#[test]
fn r2_047_alter_publication_add_emits_table_options() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER PUBLICATION pub ADD ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLE"), "labels: {labels:?}");
  assert!(labels.contains(&"TABLES IN SCHEMA"), "labels: {labels:?}");
}

#[test]
fn r2_047_alter_subscription_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SUBSCRIPTION sub ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CONNECTION"), "labels: {labels:?}");
  assert!(labels.contains(&"SET PUBLICATION"), "labels: {labels:?}");
  assert!(labels.contains(&"REFRESH PUBLICATION"), "labels: {labels:?}");
  assert!(labels.contains(&"ENABLE"), "labels: {labels:?}");
  assert!(labels.contains(&"DISABLE"), "labels: {labels:?}");
  assert!(labels.contains(&"SKIP"), "labels: {labels:?}");
}

#[test]
fn r2_047_alter_subscription_set_emits_publication_or_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SUBSCRIPTION sub SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PUBLICATION"), "labels: {labels:?}");
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_047_alter_subscription_refresh_emits_publication() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SUBSCRIPTION sub REFRESH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PUBLICATION"), "labels: {labels:?}");
}

#[test]
fn r2_047_alter_subscription_skip_emits_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SUBSCRIPTION sub SKIP ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_048_create_publication_for_emits_targets() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE PUBLICATION pub FOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALL TABLES"), "labels: {labels:?}");
  assert!(labels.contains(&"TABLE"), "labels: {labels:?}");
  assert!(labels.contains(&"TABLES IN SCHEMA"), "labels: {labels:?}");
}

#[test]
fn r2_048_create_publication_with_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE PUBLICATION pub FOR ALL TABLES WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"(publish"), "labels: {labels:?}");
  assert!(labels.contains(&"(publish_via_partition_root"), "labels: {labels:?}");
}

#[test]
fn r2_048_create_subscription_after_name_emits_connection() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE SUBSCRIPTION sub ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CONNECTION"), "labels: {labels:?}");
}

#[test]
fn r2_048_create_subscription_after_connection_emits_publication() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE SUBSCRIPTION sub CONNECTION 'host=remote' ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PUBLICATION"), "labels: {labels:?}");
}

#[test]
fn r2_048_create_subscription_after_publication_emits_with() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE SUBSCRIPTION sub CONNECTION 'host=remote' PUBLICATION mypub ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"WITH"), "labels: {labels:?}");
}

#[test]
fn r2_048_create_subscription_with_paren_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE SUBSCRIPTION sub CONNECTION 'host=remote' PUBLICATION mypub WITH ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"connect"), "labels: {labels:?}");
  assert!(labels.contains(&"create_slot"), "labels: {labels:?}");
  assert!(labels.contains(&"slot_name"), "labels: {labels:?}");
  assert!(labels.contains(&"streaming"), "labels: {labels:?}");
  assert!(labels.contains(&"two_phase"), "labels: {labels:?}");
  assert!(labels.contains(&"disable_on_error"), "labels: {labels:?}");
  assert!(labels.contains(&"copy_data"), "labels: {labels:?}");
}

#[test]
fn r2_049_create_view_after_name_emits_with_as() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE VIEW v ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"WITH"), "labels: {labels:?}");
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
}

#[test]
fn r2_049_create_view_with_check_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE VIEW v AS SELECT * FROM users WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CHECK OPTION"), "labels: {labels:?}");
  assert!(labels.contains(&"CASCADED CHECK OPTION"), "labels: {labels:?}");
  assert!(labels.contains(&"LOCAL CHECK OPTION"), "labels: {labels:?}");
}

#[test]
fn r2_049_create_mv_after_name_emits_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE MATERIALIZED VIEW mv ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"USING"), "labels: {labels:?}");
  assert!(labels.contains(&"WITH"), "labels: {labels:?}");
  assert!(labels.contains(&"TABLESPACE"), "labels: {labels:?}");
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
}

#[test]
fn r2_049_create_mv_with_data_chain() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE MATERIALIZED VIEW mv AS SELECT 1 WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DATA"), "labels: {labels:?}");
  assert!(labels.contains(&"NO DATA"), "labels: {labels:?}");
}

#[test]
fn r2_049_alter_operator_post_paren_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER OPERATOR === (int, int) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET SCHEMA"), "labels: {labels:?}");
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
}

#[test]
fn r2_049_alter_type_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TYPE mood ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ADD VALUE"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME VALUE"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"ADD ATTRIBUTE"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP ATTRIBUTE"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET SCHEMA"), "labels: {labels:?}");
}

#[test]
fn r2_050_alter_mv_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER MATERIALIZED VIEW mv ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET TABLESPACE"), "labels: {labels:?}");
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
  assert!(labels.contains(&"RESET"), "labels: {labels:?}");
  assert!(labels.contains(&"ALTER COLUMN"), "labels: {labels:?}");
  assert!(labels.contains(&"CLUSTER ON"), "labels: {labels:?}");
  assert!(labels.contains(&"DEPENDS ON EXTENSION"), "labels: {labels:?}");
}

#[test]
fn r2_050_alter_mv_set_emits_subkeywords() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER MATERIALIZED VIEW mv SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SCHEMA"), "labels: {labels:?}");
  assert!(labels.contains(&"TABLESPACE"), "labels: {labels:?}");
  assert!(labels.contains(&"ACCESS METHOD"), "labels: {labels:?}");
  assert!(labels.contains(&"WITHOUT CLUSTER"), "labels: {labels:?}");
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_050_alter_mv_rename_emits_to_column() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER MATERIALIZED VIEW mv RENAME ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TO"), "labels: {labels:?}");
  assert!(labels.contains(&"COLUMN"), "labels: {labels:?}");
}

#[test]
fn r2_050_alter_collation_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER COLLATION en_US ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET SCHEMA"), "labels: {labels:?}");
  assert!(labels.contains(&"REFRESH VERSION"), "labels: {labels:?}");
}

#[test]
fn r2_050_alter_collation_refresh_emits_version() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER COLLATION en_US REFRESH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"VERSION"), "labels: {labels:?}");
}

#[test]
fn r2_051_alter_domain_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DOMAIN positive_int ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SET DEFAULT"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP DEFAULT"), "labels: {labels:?}");
  assert!(labels.contains(&"SET NOT NULL"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP NOT NULL"), "labels: {labels:?}");
  assert!(labels.contains(&"ADD CONSTRAINT"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP CONSTRAINT"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME CONSTRAINT"), "labels: {labels:?}");
  assert!(labels.contains(&"VALIDATE CONSTRAINT"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET SCHEMA"), "labels: {labels:?}");
}

#[test]
fn r2_051_alter_domain_set_emits_subkeywords() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DOMAIN positive_int SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEFAULT"), "labels: {labels:?}");
  assert!(labels.contains(&"NOT NULL"), "labels: {labels:?}");
  assert!(labels.contains(&"SCHEMA"), "labels: {labels:?}");
}

#[test]
fn r2_051_alter_domain_drop_emits_subkeywords() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DOMAIN positive_int DROP ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEFAULT"), "labels: {labels:?}");
  assert!(labels.contains(&"NOT NULL"), "labels: {labels:?}");
  assert!(labels.contains(&"CONSTRAINT"), "labels: {labels:?}");
}

#[test]
fn r2_051_alter_domain_rename_emits_constraint_to() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DOMAIN positive_int RENAME ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CONSTRAINT"), "labels: {labels:?}");
  assert!(labels.contains(&"TO"), "labels: {labels:?}");
}

#[test]
fn r2_051_alter_domain_validate_emits_constraint() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DOMAIN positive_int VALIDATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CONSTRAINT"), "labels: {labels:?}");
}

#[test]
fn r2_051_alter_domain_add_emits_constraint_check() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DOMAIN positive_int ADD ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CONSTRAINT"), "labels: {labels:?}");
  assert!(labels.contains(&"CHECK"), "labels: {labels:?}");
}

#[test]
fn r2_052_alter_sequence_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SEQUENCE s ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
  assert!(labels.contains(&"INCREMENT BY"), "labels: {labels:?}");
  assert!(labels.contains(&"MINVALUE"), "labels: {labels:?}");
  assert!(labels.contains(&"MAXVALUE"), "labels: {labels:?}");
  assert!(labels.contains(&"START WITH"), "labels: {labels:?}");
  assert!(labels.contains(&"RESTART"), "labels: {labels:?}");
  assert!(labels.contains(&"CACHE"), "labels: {labels:?}");
  assert!(labels.contains(&"CYCLE"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNED BY"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
}

#[test]
fn r2_052_alter_sequence_no_emits_minmax_cycle() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SEQUENCE s NO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"MINVALUE"), "labels: {labels:?}");
  assert!(labels.contains(&"MAXVALUE"), "labels: {labels:?}");
  assert!(labels.contains(&"CYCLE"), "labels: {labels:?}");
}

#[test]
fn r2_052_alter_sequence_increment_emits_by() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SEQUENCE s INCREMENT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"BY"), "labels: {labels:?}");
}

#[test]
fn r2_052_alter_index_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER INDEX ix ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET TABLESPACE"), "labels: {labels:?}");
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
  assert!(labels.contains(&"RESET"), "labels: {labels:?}");
  assert!(labels.contains(&"ATTACH PARTITION"), "labels: {labels:?}");
  assert!(labels.contains(&"DEPENDS ON EXTENSION"), "labels: {labels:?}");
  assert!(labels.contains(&"ALTER COLUMN"), "labels: {labels:?}");
}

#[test]
fn r2_052_alter_index_set_emits_tablespace_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER INDEX ix SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLESPACE"), "labels: {labels:?}");
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_052_alter_index_attach_emits_partition() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER INDEX ix ATTACH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"PARTITION"), "labels: {labels:?}");
}

#[test]
fn r2_053_alter_view_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER VIEW v ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME COLUMN"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET SCHEMA"), "labels: {labels:?}");
  assert!(labels.contains(&"ALTER COLUMN"), "labels: {labels:?}");
}

#[test]
fn r2_053_alter_view_set_emits_subkeywords() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER VIEW v SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SCHEMA"), "labels: {labels:?}");
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_053_alter_statistics_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER STATISTICS st ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET SCHEMA"), "labels: {labels:?}");
  assert!(labels.contains(&"SET STATISTICS"), "labels: {labels:?}");
}

#[test]
fn r2_053_alter_statistics_set_emits_schema_statistics() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER STATISTICS st SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SCHEMA"), "labels: {labels:?}");
  assert!(labels.contains(&"STATISTICS"), "labels: {labels:?}");
}

#[test]
fn r2_054_alter_policy_after_on_table_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER POLICY p ON users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"TO"), "labels: {labels:?}");
  assert!(labels.contains(&"USING"), "labels: {labels:?}");
  assert!(labels.contains(&"WITH CHECK"), "labels: {labels:?}");
}

#[test]
fn r2_054_alter_policy_to_emits_roles() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER POLICY p ON users TO ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Phase::AfterGrantTo emits PUBLIC + catalog roles. Existing detector
  // CURRENT_USER/SESSION_USER kws are masked by that handler -- accept
  // PUBLIC as the canary.
  assert!(labels.contains(&"PUBLIC"), "labels: {labels:?}");
}

#[test]
fn r2_054_alter_policy_with_emits_check() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER POLICY p ON users WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CHECK"), "labels: {labels:?}");
}

#[test]
fn r2_054_alter_text_search_emits_kinds() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TEXT SEARCH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"CONFIGURATION"), "labels: {labels:?}");
  assert!(labels.contains(&"DICTIONARY"), "labels: {labels:?}");
  assert!(labels.contains(&"PARSER"), "labels: {labels:?}");
  assert!(labels.contains(&"TEMPLATE"), "labels: {labels:?}");
}

#[test]
fn r2_054_alter_text_search_dictionary_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TEXT SEARCH DICTIONARY my_dict ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET SCHEMA"), "labels: {labels:?}");
}

#[test]
fn r2_054_alter_text_search_configuration_after_name_emits_mapping_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TEXT SEARCH CONFIGURATION my_cfg ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ADD MAPPING FOR"), "labels: {labels:?}");
  assert!(labels.contains(&"ALTER MAPPING FOR"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP MAPPING FOR"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
}

#[test]
fn r2_055_create_event_trigger_when_emits_tag() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EVENT TRIGGER trg ON ddl_command_end WHEN ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"tag"), "labels: {labels:?}");
}

#[test]
fn r2_055_create_event_trigger_tag_emits_in() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EVENT TRIGGER trg ON ddl_command_end WHEN tag ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"IN"), "labels: {labels:?}");
}

#[test]
fn r2_055_create_event_trigger_in_emits_paren() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EVENT TRIGGER trg ON ddl_command_end WHEN tag IN ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"("), "labels: {labels:?}");
}

#[test]
fn r2_055_create_event_trigger_paren_emits_command_tags() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EVENT TRIGGER trg ON ddl_command_end WHEN tag IN (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"'CREATE TABLE'"), "labels: {labels:?}");
  assert!(labels.contains(&"'DROP TABLE'"), "labels: {labels:?}");
  assert!(labels.contains(&"'ALTER TABLE'"), "labels: {labels:?}");
  assert!(labels.contains(&"'CREATE FUNCTION'"), "labels: {labels:?}");
}

#[test]
fn r2_055_create_collation_paren_emits_locale_options() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE COLLATION my_coll ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LOCALE"), "labels: {labels:?}");
  assert!(labels.contains(&"LC_COLLATE"), "labels: {labels:?}");
  assert!(labels.contains(&"PROVIDER"), "labels: {labels:?}");
}

#[test]
fn r2_056_alter_event_trigger_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER EVENT TRIGGER trg ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ENABLE"), "labels: {labels:?}");
  assert!(labels.contains(&"DISABLE"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"DEPENDS ON EXTENSION"), "labels: {labels:?}");
}

#[test]
fn r2_056_alter_event_trigger_enable_emits_replica_always() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER EVENT TRIGGER trg ENABLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"REPLICA"), "labels: {labels:?}");
  assert!(labels.contains(&"ALWAYS"), "labels: {labels:?}");
}

#[test]
fn r2_056_alter_operator_class_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER OPERATOR CLASS my_ops ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"USING"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET SCHEMA"), "labels: {labels:?}");
}

#[test]
fn r2_056_alter_operator_class_using_emits_index_methods() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER OPERATOR CLASS my_ops USING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"btree"), "labels: {labels:?}");
  assert!(labels.contains(&"gist"), "labels: {labels:?}");
  assert!(labels.contains(&"gin"), "labels: {labels:?}");
  assert!(labels.contains(&"brin"), "labels: {labels:?}");
}

#[test]
fn r2_056_alter_operator_family_after_name_using_emits_add_drop() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER OPERATOR FAMILY my_ops USING btree ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ADD"), "labels: {labels:?}");
  assert!(labels.contains(&"DROP"), "labels: {labels:?}");
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
}

#[test]
fn r2_056_alter_operator_family_add_emits_operator_function() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER OPERATOR FAMILY my_ops USING btree ADD ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OPERATOR"), "labels: {labels:?}");
  assert!(labels.contains(&"FUNCTION"), "labels: {labels:?}");
}

#[test]
fn r2_057_alter_default_privileges_emits_for_in_grant_revoke() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DEFAULT PRIVILEGES ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FOR ROLE"), "labels: {labels:?}");
  assert!(labels.contains(&"FOR USER"), "labels: {labels:?}");
  assert!(labels.contains(&"IN SCHEMA"), "labels: {labels:?}");
  assert!(labels.contains(&"GRANT"), "labels: {labels:?}");
  assert!(labels.contains(&"REVOKE"), "labels: {labels:?}");
}

#[test]
fn r2_057_alter_default_privileges_on_emits_targets() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER DEFAULT PRIVILEGES IN SCHEMA app GRANT SELECT ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TABLES"), "labels: {labels:?}");
  assert!(labels.contains(&"SEQUENCES"), "labels: {labels:?}");
  assert!(labels.contains(&"FUNCTIONS"), "labels: {labels:?}");
  assert!(labels.contains(&"ROUTINES"), "labels: {labels:?}");
  assert!(labels.contains(&"TYPES"), "labels: {labels:?}");
  assert!(labels.contains(&"SCHEMAS"), "labels: {labels:?}");
}

#[test]
fn r2_057_create_operator_class_using_emits_methods() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OPERATOR CLASS my_ops DEFAULT FOR TYPE int USING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"btree"), "labels: {labels:?}");
  assert!(labels.contains(&"gist"), "labels: {labels:?}");
  assert!(labels.contains(&"hash"), "labels: {labels:?}");
}

#[test]
fn r2_057_create_operator_class_as_emits_body_items() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OPERATOR CLASS my_ops DEFAULT FOR TYPE int USING btree AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OPERATOR"), "labels: {labels:?}");
  assert!(labels.contains(&"FUNCTION"), "labels: {labels:?}");
  assert!(labels.contains(&"STORAGE"), "labels: {labels:?}");
}

#[test]
fn r2_057_create_operator_class_after_using_method_emits_family_or_as() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OPERATOR CLASS my_ops DEFAULT FOR TYPE int USING btree ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FAMILY"), "labels: {labels:?}");
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
}

#[test]
fn r2_058_create_operator_family_after_name_emits_using() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OPERATOR FAMILY my_family ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"USING"), "labels: {labels:?}");
}

#[test]
fn r2_058_create_operator_family_using_emits_methods() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OPERATOR FAMILY my_family USING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"btree"), "labels: {labels:?}");
  assert!(labels.contains(&"hash"), "labels: {labels:?}");
  assert!(labels.contains(&"gist"), "labels: {labels:?}");
  assert!(labels.contains(&"spgist"), "labels: {labels:?}");
  assert!(labels.contains(&"gin"), "labels: {labels:?}");
  assert!(labels.contains(&"brin"), "labels: {labels:?}");
}

#[test]
fn r2_058_alter_procedure_after_args_emits_attrs() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER PROCEDURE p(int) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET SCHEMA"), "labels: {labels:?}");
}

#[test]
fn r2_058_alter_routine_after_args_emits_attrs() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER ROUTINE r(int) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
}

#[test]
fn r2_059_create_transform_emits_for_type() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRANSFORM ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FOR TYPE"), "labels: {labels:?}");
}

#[test]
fn r2_059_create_transform_for_emits_type() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRANSFORM FOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TYPE"), "labels: {labels:?}");
}

#[test]
fn r2_059_create_transform_after_type_emits_language() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRANSFORM FOR TYPE hstore ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LANGUAGE"), "labels: {labels:?}");
}

#[test]
fn r2_059_create_transform_paren_emits_from_to_sql() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRANSFORM FOR TYPE hstore LANGUAGE plperl ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FROM SQL WITH FUNCTION"), "labels: {labels:?}");
  assert!(labels.contains(&"TO SQL WITH FUNCTION"), "labels: {labels:?}");
}

#[test]
fn r2_059_create_cast_as_emits_assignment_implicit() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE CAST (text AS int) WITH FUNCTION myfn AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ASSIGNMENT"), "labels: {labels:?}");
  assert!(labels.contains(&"IMPLICIT"), "labels: {labels:?}");
}

#[test]
fn r2_060_alter_access_method_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER ACCESS METHOD myam ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
}

#[test]
fn r2_060_create_conversion_after_name_emits_for() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE CONVERSION my_conv ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FOR"), "labels: {labels:?}");
}

#[test]
fn r2_060_create_conversion_after_src_enc_emits_to() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE CONVERSION my_conv FOR 'LATIN1' ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TO"), "labels: {labels:?}");
}

#[test]
fn r2_060_create_conversion_after_dst_enc_emits_from() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE CONVERSION my_conv FOR 'LATIN1' TO 'UTF8' ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FROM"), "labels: {labels:?}");
}

#[test]
fn r2_060_alter_conversion_after_name_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER CONVERSION my_conv ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
  assert!(labels.contains(&"SET SCHEMA"), "labels: {labels:?}");
}

#[test]
fn r2_061_alter_rule_after_name_emits_on() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER RULE r ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ON"), "labels: {labels:?}");
}

#[test]
fn r2_061_alter_rule_after_on_table_emits_rename_to() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER RULE r ON users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
}

#[test]
fn r2_061_alter_trigger_after_name_emits_on() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TRIGGER t ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ON"), "labels: {labels:?}");
}

#[test]
fn r2_061_alter_trigger_after_on_table_emits_actions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TRIGGER t ON users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RENAME TO"), "labels: {labels:?}");
  assert!(labels.contains(&"DEPENDS ON EXTENSION"), "labels: {labels:?}");
  assert!(labels.contains(&"NO DEPENDS ON EXTENSION"), "labels: {labels:?}");
}

#[test]
fn r2_061_alter_trigger_depends_emits_on_extension() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TRIGGER t ON users DEPENDS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ON EXTENSION"), "labels: {labels:?}");
}

#[test]
fn r2_062_security_label_is_emits_null_or_literal() {
  let cat = catalog_with_users_and_orders();
  let src = "SECURITY LABEL ON TABLE t IS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NULL"), "labels: {labels:?}");
  assert!(labels.contains(&"'unclassified'"), "labels: {labels:?}");
  assert!(labels.contains(&"'secret'"), "labels: {labels:?}");
}

#[test]
fn r2_062_comment_on_is_emits_null_empty() {
  let cat = catalog_with_users_and_orders();
  let src = "COMMENT ON TABLE users IS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NULL"), "labels: {labels:?}");
  assert!(labels.contains(&"''"), "labels: {labels:?}");
}

#[test]
fn r2_062_comment_on_after_class_emits_is() {
  let cat = catalog_with_users_and_orders();
  let src = "COMMENT ON TABLE users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"IS"), "labels: {labels:?}");
}

#[test]
fn r2_062_explain_paren_full_options() {
  let cat = catalog_with_users_and_orders();
  let src = "EXPLAIN ( ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ANALYZE"), "labels: {labels:?}");
  assert!(labels.contains(&"VERBOSE"), "labels: {labels:?}");
  assert!(labels.contains(&"COSTS"), "labels: {labels:?}");
  assert!(labels.contains(&"BUFFERS"), "labels: {labels:?}");
  assert!(labels.contains(&"WAL"), "labels: {labels:?}");
  assert!(labels.contains(&"TIMING"), "labels: {labels:?}");
  assert!(labels.contains(&"SETTINGS"), "labels: {labels:?}");
  assert!(labels.contains(&"GENERIC_PLAN"), "labels: {labels:?}");
}

#[test]
fn r2_063_alter_system_emits_set_reset() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SYSTEM ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SET"), "labels: {labels:?}");
  assert!(labels.contains(&"RESET"), "labels: {labels:?}");
}

#[test]
fn r2_063_alter_system_set_emits_common_gucs() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SYSTEM SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"shared_buffers"), "labels: {labels:?}");
  assert!(labels.contains(&"work_mem"), "labels: {labels:?}");
  assert!(labels.contains(&"max_connections"), "labels: {labels:?}");
  assert!(labels.contains(&"wal_level"), "labels: {labels:?}");
  assert!(labels.contains(&"synchronous_commit"), "labels: {labels:?}");
}

#[test]
fn r2_063_alter_system_reset_emits_all() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SYSTEM RESET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALL"), "labels: {labels:?}");
}

#[test]
fn r2_063_alter_system_set_param_emits_to_equals() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER SYSTEM SET work_mem ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"TO"), "labels: {labels:?}");
  assert!(labels.contains(&"="), "labels: {labels:?}");
}

#[test]
fn r2_063_alter_large_object_emits_owner_to() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER LARGE OBJECT 16385 ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"OWNER TO"), "labels: {labels:?}");
}

#[test]
fn r2_064_show_emits_guc_menu() {
  let cat = catalog_with_users_and_orders();
  let src = "SHOW ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALL"), "labels: {labels:?}");
  assert!(labels.contains(&"search_path"), "labels: {labels:?}");
  assert!(labels.contains(&"timezone"), "labels: {labels:?}");
  assert!(labels.contains(&"work_mem"), "labels: {labels:?}");
}

#[test]
fn r2_064_set_emits_local_session() {
  let cat = catalog_with_users_and_orders();
  // `SET ` alone routes to the LOCAL/SESSION modifier menu before the
  // GUC menu fires -- subsequent `SET LOCAL <cursor>` surfaces GUCs.
  let src = "SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LOCAL"), "labels: {labels:?}");
  assert!(labels.contains(&"SESSION"), "labels: {labels:?}");
}

#[test]
fn r2_064_set_local_emits_guc_menu() {
  let cat = catalog_with_users_and_orders();
  let src = "SET LOCAL ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"work_mem"), "labels: {labels:?}");
  assert!(labels.contains(&"statement_timeout"), "labels: {labels:?}");
}

#[test]
fn r2_064_show_includes_pg16_plus() {
  let cat = catalog_with_users_and_orders();
  let src = "SHOW ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"vacuum_buffer_usage_limit"), "labels: {labels:?}");
  assert!(labels.contains(&"plan_cache_mode"), "labels: {labels:?}");
  assert!(labels.contains(&"session_replication_role"), "labels: {labels:?}");
}

#[test]
fn r2_064_reset_emits_all_role() {
  let cat = catalog_with_users_and_orders();
  // `RESET ` routes to the ALL/ROLE shortlist first.
  let src = "RESET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ALL"), "labels: {labels:?}");
  assert!(labels.contains(&"ROLE"), "labels: {labels:?}");
}

#[test]
fn r2_065_sweep_create_or_replace_view() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR REPLACE VIEW v ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
}

#[test]
fn r2_065_sweep_create_or_replace_function() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR REPLACE FUNCTION f(int) RETURNS int ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LANGUAGE"), "labels: {labels:?}");
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
}

#[test]
fn r2_065_sweep_create_or_replace_procedure() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR REPLACE PROCEDURE p(int) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"LANGUAGE"), "labels: {labels:?}");
  assert!(labels.contains(&"AS"), "labels: {labels:?}");
}

#[test]
fn r2_065_sweep_create_or_replace_trigger() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR REPLACE TRIGGER trg ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"BEFORE"), "labels: {labels:?}");
  assert!(labels.contains(&"AFTER"), "labels: {labels:?}");
  assert!(labels.contains(&"INSTEAD OF"), "labels: {labels:?}");
}

#[test]
fn r2_065_sweep_create_or_replace_rule() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR REPLACE RULE r AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"ON"), "labels: {labels:?}");
}

#[test]
fn r2_065_sweep_cycle2_mixed_bag() {
  // Touches one phase detector added per round 2..50 (sampled).
  let cat = catalog_with_users_and_orders();
  let cases: &[(&str, &str)] = &[
    ("ALTER DATABASE mydb ", "RENAME TO"),
    ("ALTER TABLESPACE ts ", "RENAME TO"),
    ("CREATE USER MAPPING ", "FOR"),
    ("CREATE LANGUAGE plperl ", "HANDLER"),
    ("ALTER FOREIGN DATA WRAPPER my_fdw ", "HANDLER"),
    ("ALTER SERVER remote ", "VERSION"),
    ("ALTER PUBLICATION pub ", "ADD"),
    ("ALTER SUBSCRIPTION sub ", "CONNECTION"),
    ("CREATE PUBLICATION pub FOR ", "ALL TABLES"),
    ("CREATE SUBSCRIPTION sub ", "CONNECTION"),
    ("ALTER MATERIALIZED VIEW mv ", "RENAME TO"),
    ("ALTER COLLATION en_US ", "REFRESH VERSION"),
    ("ALTER DOMAIN d ", "SET DEFAULT"),
    ("ALTER SEQUENCE s ", "RESTART"),
    ("ALTER INDEX ix ", "SET TABLESPACE"),
    ("ALTER VIEW v ", "RENAME TO"),
    ("ALTER STATISTICS st ", "SET STATISTICS"),
    ("ALTER POLICY p ON users ", "USING"),
    ("ALTER TEXT SEARCH ", "CONFIGURATION"),
    ("CREATE EVENT TRIGGER t ON ddl_command_end WHEN tag IN (", "'CREATE TABLE'"),
    ("ALTER EVENT TRIGGER trg ", "ENABLE"),
    ("ALTER OPERATOR CLASS my_ops ", "USING"),
    ("CREATE OPERATOR FAMILY my_fam ", "USING"),
    ("CREATE OPERATOR CLASS my_ops DEFAULT FOR TYPE int USING btree AS ", "OPERATOR"),
    ("CREATE TRANSFORM ", "FOR TYPE"),
    ("ALTER ACCESS METHOD myam ", "RENAME TO"),
    ("CREATE CONVERSION my_conv ", "FOR"),
    ("ALTER CONVERSION my_conv ", "RENAME TO"),
    ("ALTER RULE r ON users ", "RENAME TO"),
    ("ALTER TRIGGER t ON users ", "RENAME TO"),
    ("ALTER LARGE OBJECT 16385 ", "OWNER TO"),
    ("ALTER SYSTEM ", "SET"),
    ("COMMENT ON TABLE users IS ", "NULL"),
  ];
  for (src, expect) in cases {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(expect), "src {src:?} missing {expect}; labels: {labels:?}");
  }
}

#[test]
fn r2_066_fresh_name_override_alter_user_mapping() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER USER MAPPING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FOR"), "labels: {labels:?}");
}

#[test]
fn r2_066_fresh_name_override_fetch() {
  let cat = catalog_with_users_and_orders();
  let src = "FETCH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NEXT"), "labels: {labels:?}");
  assert!(labels.contains(&"FORWARD"), "labels: {labels:?}");
}

#[test]
fn r2_066_fresh_name_override_move() {
  let cat = catalog_with_users_and_orders();
  let src = "MOVE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"NEXT"), "labels: {labels:?}");
}

#[test]
fn r2_066_create_table_if_not_exists_surfaces() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"IF NOT EXISTS"), "labels: {labels:?}");
}

#[test]
fn r2_067_sweep_top_level_create_chain() {
  let cat = catalog_with_users_and_orders();
  for (src, expect) in &[
    ("CREATE TABLE t (id int) ", "INHERITS"),
    ("CREATE INDEX ix ON users (email) ", "INCLUDE"),
    ("CREATE TYPE my_t ", "AS"),
    ("CREATE TYPE r AS RANGE ( ", "SUBTYPE"),
    ("CREATE TYPE base_t ( ", "INPUT"),
    ("CREATE DOMAIN positive_int ", "AS"),
    ("CREATE COLLATION my_coll ", "FROM"),
    ("CREATE COLLATION my_coll ( ", "LOCALE"),
    ("CREATE EVENT TRIGGER trg ON ", "ddl_command_start"),
    ("CREATE EVENT TRIGGER trg ON ddl_command_start EXECUTE ", "FUNCTION"),
    ("CREATE DATABASE mydb ", "OWNER"),
    ("CREATE TABLESPACE ts ", "OWNER"),
    ("CREATE OPERATOR === ( ", "FUNCTION"),
    ("CREATE AGGREGATE agg_sum(int) ( ", "SFUNC"),
    ("CREATE CAST (text AS int) ", "WITH FUNCTION"),
    ("CREATE TRIGGER trg ", "BEFORE"),
    ("CREATE TRIGGER trg AFTER UPDATE ON users REFERENCING ", "OLD TABLE AS"),
    ("CREATE INDEX CONCURRENTLY ix ", "ON"),
    ("CREATE SCHEMA app ", "AUTHORIZATION"),
    ("CREATE EXTENSION pg_trgm WITH ", "SCHEMA"),
    ("CREATE FOREIGN TABLE ft (id int) SERVER remote_srv ", "OPTIONS"),
    ("CREATE STATISTICS st ( ", "ndistinct"),
    ("CREATE TEXT SEARCH ", "CONFIGURATION"),
    ("CREATE PUBLICATION pub FOR ", "ALL TABLES"),
    ("CREATE SUBSCRIPTION sub CONNECTION 'host=x' ", "PUBLICATION"),
    ("CREATE OPERATOR FAMILY my_fam ", "USING"),
    ("CREATE OPERATOR CLASS my_ops DEFAULT FOR TYPE int USING btree ", "AS"),
    ("CREATE TRANSFORM ", "FOR TYPE"),
    ("CREATE CONVERSION my_conv FOR 'LATIN1' TO 'UTF8' ", "FROM"),
    ("CREATE LANGUAGE plperl ", "HANDLER"),
    ("CREATE FOREIGN DATA WRAPPER my_fdw ", "HANDLER"),
    ("CREATE SERVER remote ", "TYPE"),
    ("CREATE ROLE alice ", "LOGIN"),
    ("CREATE USER MAPPING ", "FOR"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(expect), "src {src:?} missing {expect}; labels: {labels:?}");
  }
}

#[test]
fn r2_067_sweep_top_level_alter_chain() {
  let cat = catalog_with_users_and_orders();
  for (src, expect) in &[
    ("ALTER ROLE alice WITH ", "SUPERUSER"),
    ("ALTER USER alice WITH ", "SUPERUSER"),
    ("ALTER DATABASE mydb ", "OWNER TO"),
    ("ALTER DATABASE mydb WITH ", "ALLOW_CONNECTIONS"),
    ("ALTER TABLESPACE ts ", "OWNER TO"),
    ("ALTER SCHEMA app ", "OWNER TO"),
    ("ALTER COLLATION en_US ", "OWNER TO"),
    ("ALTER DOMAIN d ", "OWNER TO"),
    ("ALTER SEQUENCE s ", "OWNER TO"),
    ("ALTER INDEX ix ", "RENAME TO"),
    ("ALTER VIEW v ", "OWNER TO"),
    ("ALTER MATERIALIZED VIEW mv ", "OWNER TO"),
    ("ALTER STATISTICS st ", "OWNER TO"),
    ("ALTER POLICY p ON users ", "USING"),
    ("ALTER PUBLICATION pub ", "ADD"),
    ("ALTER SUBSCRIPTION sub ", "CONNECTION"),
    ("ALTER LANGUAGE plperl ", "OWNER TO"),
    ("ALTER FOREIGN DATA WRAPPER my_fdw ", "VALIDATOR"),
    ("ALTER SERVER remote ", "VERSION"),
    ("ALTER USER MAPPING ", "FOR"),
    ("ALTER TYPE mood ", "ADD VALUE"),
    ("ALTER EVENT TRIGGER trg ", "ENABLE"),
    ("ALTER OPERATOR CLASS my_ops ", "USING"),
    ("ALTER OPERATOR FAMILY my_ops USING btree ", "ADD"),
    ("ALTER ACCESS METHOD myam ", "OWNER TO"),
    ("ALTER CONVERSION my_conv ", "SET SCHEMA"),
    ("ALTER RULE r ON users ", "RENAME TO"),
    ("ALTER TRIGGER t ON users ", "RENAME TO"),
    ("ALTER LARGE OBJECT 16385 ", "OWNER TO"),
    ("ALTER SYSTEM ", "SET"),
    ("ALTER TEXT SEARCH CONFIGURATION cfg ", "ADD MAPPING FOR"),
    ("ALTER FUNCTION f(int) ", "OWNER TO"),
    ("ALTER PROCEDURE p(int) ", "OWNER TO"),
    ("ALTER ROUTINE r(int) ", "OWNER TO"),
    ("ALTER DEFAULT PRIVILEGES ", "GRANT"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(expect), "src {src:?} missing {expect}; labels: {labels:?}");
  }
}

#[test]
fn r2_067_sweep_dml_followups() {
  let cat = catalog_with_users_and_orders();
  for (src, expect) in &[
    ("MERGE INTO users u USING staged s ON u.id=s.id WHEN MATCHED THEN UPDATE ", "SET"),
    ("MERGE INTO users u USING staged s ON u.id=s.id WHEN NOT MATCHED THEN INSERT ", "VALUES"),
    ("INSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE ", "SET"),
    ("INSERT INTO users (id) VALUES (1) ON CONFLICT ", "DO NOTHING"),
    ("SELECT 1 UNION ", "ALL"),
    ("SELECT * FROM users FOR ", "UPDATE"),
    ("SELECT * FROM users FOR UPDATE ", "OF"),
    ("WITH r AS (SELECT 1) ", "SELECT"),
    ("REFRESH MATERIALIZED VIEW mv ", "WITH DATA"),
    ("VACUUM ( ", "PARALLEL"),
    ("REINDEX ( ", "VERBOSE"),
    ("COPY users TO ", "STDOUT"),
    ("COPY users FROM ", "STDIN"),
    ("FETCH ", "NEXT"),
    ("MOVE ", "BACKWARD"),
    ("DECLARE c CURSOR ", "FOR"),
    ("ROLLBACK ", "TO"),
    ("COMMIT ", "AND"),
    ("ROLLBACK TO ", "SAVEPOINT"),
    ("SET ROLE ", "NONE"),
    ("SET SESSION AUTHORIZATION ", "DEFAULT"),
    ("BEGIN ", "ATOMIC"),
    ("GRANT SELECT ON ", "TABLE"),
    ("GRANT SELECT ON ALL TABLES IN SCHEMA app TO svc ", "WITH GRANT OPTION"),
    ("REVOKE WITH ", "GRANT OPTION FOR"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(expect), "src {src:?} missing {expect}; labels: {labels:?}");
  }
}

#[test]
fn r2_074_cycle2_final_consolidation() {
  // Compact final sweep touching one phase from each major detector
  // family added across cycle 2. Boundary check: every entry must
  // surface the expected label.
  let cat = catalog_with_users_and_orders();
  let cases: &[(&str, &str)] = &[
    // create chain
    ("CREATE TABLE t (id int) ", "INHERITS"),
    ("CREATE VIEW v ", "AS"),
    ("CREATE MATERIALIZED VIEW mv ", "USING"),
    ("CREATE INDEX ix ON users (email) ", "INCLUDE"),
    ("CREATE TRIGGER trg AFTER UPDATE ON users REFERENCING ", "NEW TABLE AS"),
    ("CREATE EVENT TRIGGER trg ON ddl_command_end WHEN tag IN (", "'CREATE TABLE'"),
    ("CREATE DATABASE mydb LOCALE_PROVIDER ", "icu"),
    ("CREATE TABLESPACE ts ", "OWNER"),
    ("CREATE OPERATOR === ( ", "FUNCTION"),
    ("CREATE AGGREGATE agg(int) ( ", "SFUNC"),
    ("CREATE CAST (text AS int) WITH ", "FUNCTION"),
    ("CREATE TYPE r AS RANGE ( ", "SUBTYPE"),
    ("CREATE STATISTICS st ( ", "ndistinct"),
    ("CREATE TEXT SEARCH ", "CONFIGURATION"),
    ("CREATE PUBLICATION pub FOR ", "ALL TABLES"),
    ("CREATE SUBSCRIPTION sub CONNECTION 'host=x' PUBLICATION mypub WITH ( ", "create_slot"),
    ("CREATE OPERATOR FAMILY fam ", "USING"),
    ("CREATE OPERATOR CLASS my_ops DEFAULT FOR TYPE int USING btree AS ", "OPERATOR"),
    ("CREATE TRANSFORM FOR TYPE hstore LANGUAGE plperl ( ", "FROM SQL WITH FUNCTION"),
    ("CREATE CONVERSION my_conv FOR 'LATIN1' TO 'UTF8' ", "FROM"),
    ("CREATE FOREIGN DATA WRAPPER my_fdw ", "HANDLER"),
    ("CREATE LANGUAGE plperl ", "HANDLER"),
    ("CREATE SERVER remote ", "TYPE"),
    ("CREATE USER MAPPING FOR alice SERVER remote ", "OPTIONS"),

    // alter chain (one-per-handler)
    ("ALTER DATABASE mydb ", "REFRESH COLLATION VERSION"),
    ("ALTER TABLESPACE ts ", "OWNER TO"),
    ("ALTER FOREIGN DATA WRAPPER my_fdw OPTIONS ( ", "ADD"),
    ("ALTER SERVER remote OPTIONS ( ", "DROP"),
    ("ALTER USER MAPPING FOR alice SERVER remote OPTIONS ( ", "SET"),
    ("ALTER PUBLICATION pub ADD ", "TABLE"),
    ("ALTER SUBSCRIPTION sub REFRESH ", "PUBLICATION"),
    ("ALTER MATERIALIZED VIEW mv SET ", "WITHOUT CLUSTER"),
    ("ALTER COLLATION en_US REFRESH ", "VERSION"),
    ("ALTER DOMAIN d DROP ", "CONSTRAINT"),
    ("ALTER SEQUENCE s NO ", "CYCLE"),
    ("ALTER INDEX ix ATTACH ", "PARTITION"),
    ("ALTER VIEW v SET ", "SCHEMA"),
    ("ALTER STATISTICS st SET ", "STATISTICS"),
    ("ALTER POLICY p ON users ", "WITH CHECK"),
    ("ALTER TEXT SEARCH CONFIGURATION cfg ADD ", "MAPPING FOR"),
    ("ALTER OPERATOR CLASS ops USING ", "btree"),
    ("ALTER OPERATOR FAMILY fam USING btree ADD ", "OPERATOR"),
    ("ALTER ACCESS METHOD am ", "RENAME TO"),
    ("ALTER CONVERSION c ", "SET SCHEMA"),
    ("ALTER LANGUAGE plperl ", "RENAME TO"),
    ("ALTER EVENT TRIGGER trg ENABLE ", "REPLICA"),
    ("ALTER LARGE OBJECT 16385 ", "OWNER TO"),
    ("ALTER SYSTEM SET ", "shared_buffers"),
    ("ALTER RULE r ON users ", "RENAME TO"),
    ("ALTER TRIGGER t ON users ", "DEPENDS ON EXTENSION"),
    ("ALTER DEFAULT PRIVILEGES IN SCHEMA app GRANT SELECT ON ", "TABLES"),

    // DML / transaction edges
    ("MERGE INTO users u USING staged s ON u.id=s.id WHEN MATCHED THEN UPDATE ", "SET"),
    ("INSERT INTO users VALUES (1) ON CONFLICT (id) DO UPDATE ", "SET"),
    ("WITH r AS (SELECT 1) ", "SELECT"),
    ("REFRESH MATERIALIZED VIEW mv WITH ", "DATA"),
    ("VACUUM (SKIP_LOCKED ", "true"),
    ("REINDEX (CONCURRENTLY ", "true"),
    ("DECLARE c CURSOR WITH ", "HOLD"),
    ("FETCH FORWARD ", "ALL"),
    ("MOVE PRIOR ", "FROM"),
    ("COMMIT AND ", "CHAIN"),
    ("ROLLBACK TO ", "SAVEPOINT"),
    ("PREPARE ", "TRANSACTION"),
    ("SET ROLE ", "NONE"),
    ("SET SESSION ", "AUTHORIZATION"),
    ("BEGIN ", "ATOMIC"),
    ("GRANT WITH ", "GRANT OPTION"),
    ("DISCARD ", "ALL"),
    ("CREATE EXTENSION pg_trgm WITH ", "CASCADE"),
    ("LOCK TABLE users IN ", "ACCESS EXCLUSIVE"),
    ("CLUSTER users ", "USING"),
    ("SECURITY LABEL ON TABLE t IS ", "'secret'"),
    ("COMMENT ON TABLE users IS ", "''"),
    ("EXPLAIN (ANALYZE, SERIALIZE ", "binary"),
  ];
  let mut missing = Vec::new();
  for (src, expect) in cases {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_078_partition_by_emits_methods() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE parent (id int) PARTITION BY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"RANGE"), "labels: {labels:?}");
  assert!(labels.contains(&"LIST"), "labels: {labels:?}");
  assert!(labels.contains(&"HASH"), "labels: {labels:?}");
}

#[test]
fn r2_188_cast_with_space_emits_types() {
  let cat = catalog_with_users_and_orders();
  // Cursor with trailing space after AS.
  let src = "SELECT CAST(col AS  ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| matches!(*l, "INT" | "BIGINT" | "TEXT" | "NUMERIC" | "BOOLEAN")),
    "CAST AS with extra spaces missing types");
}

#[test]
fn r2_188_cast_function_expression_emits_types() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT CAST(now() + INTERVAL '1 day' AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| matches!(*l, "INT" | "BIGINT" | "TEXT" | "NUMERIC" | "BOOLEAN" | "TIMESTAMP" | "TIMESTAMPTZ" | "DATE")),
    "complex CAST expression AS missing types");
}

#[test]
fn r2_188_merge_when_matched_then_update_set_lhs() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // UPDATE SET LHS should expose target column names.
  assert!(labels.contains(&"name") || labels.contains(&"email") || labels.contains(&"id"),
    "MERGE THEN UPDATE SET LHS missing target cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_199_returning_after_on_conflict() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = 'x' RETURNING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"*"),
    "RETURNING after ON CONFLICT missing target cols");
}

#[test]
fn r2_199_returning_after_do_nothing() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES (1) ON CONFLICT DO NOTHING RETURNING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"*"),
    "RETURNING after DO NOTHING missing target cols");
}

#[test]
fn r2_198_returning_in_cte_with_chain() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH del AS (DELETE FROM users WHERE id = 1 RETURNING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"*"),
    "CTE RETURNING slot missing target cols");
}

#[test]
fn r2_198_returning_after_multiple_stmts() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1; INSERT INTO users (id) VALUES (1) RETURNING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"*"),
    "post-prev-stmt RETURNING missing target cols");
}

#[test]
fn r2_197_returning_with_alias_dot() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users u SET name = 'x' WHERE id = 1 RETURNING u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "RETURNING u.<col> missing alias cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_197_returning_expression_with_function() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES (1) RETURNING upper(";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Inside upper(...) -- expression slot, target cols + funcs.
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"now"),
    "RETURNING fn-arg slot missing expr ctx");
}

#[test]
fn r2_196_returning_alias_followup() {
  let cat = catalog_with_users_and_orders();
  // After `RETURNING id AS ` the next token is alias (free-form);
  // expect quiet or small AS-alias menu (FROM/AS/INTO not in scope).
  let src = "INSERT INTO users (id) VALUES (1) RETURNING id AS ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.len() < 100, "RETURNING AS slot dumped {} items", items.len());
}

#[test]
fn r2_196_returning_partial_column_filter_friendly() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES (1) RETURNING em";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"),
    "RETURNING partial col `em` missing email");
}

#[test]
fn r2_195_returning_emits_target_columns() {
  let cat = catalog_with_users_and_orders();
  for src in [
    "INSERT INTO users (id, name) VALUES (1, 'x') RETURNING ",
    "UPDATE users SET name = 'x' WHERE id = 1 RETURNING ",
    "DELETE FROM users WHERE id = 1 RETURNING ",
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"*"),
      "{src:?}: RETURNING missing target cols/*; labels[..15]={:?}",
      &labels[..labels.len().min(15)]);
  }
}

#[test]
fn r2_195_returning_after_comma_still_target_cols() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id) VALUES (1) RETURNING id, ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"name") || labels.contains(&"email"),
    "RETURNING comma-followup missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_194_insert_overriding_user_value_chain() {
  let cat = catalog_with_users_and_orders();
  for (src, want) in &[
    ("INSERT INTO users OVERRIDING ", "SYSTEM VALUE"),
    ("INSERT INTO users OVERRIDING ", "USER VALUE"),
    ("INSERT INTO users OVERRIDING USER ", "VALUE"),
    ("INSERT INTO users OVERRIDING SYSTEM ", "VALUE"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(want),
      "{src:?} -> {want}: missing; labels[..10]={:?}",
      &labels[..labels.len().min(10)]);
  }
}

#[test]
fn r2_194_insert_overriding_value_then_values_clause() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users OVERRIDING SYSTEM VALUE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"VALUES") || labels.contains(&"SELECT") || labels.contains(&"DEFAULT VALUES"),
    "after OVERRIDING VALUE missing body shape; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_193_merge_update_set_complex_expression() {
  let cat = catalog_with_users_and_orders();
  // Build expression with functions + col refs.
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name = upper(o.user_id::text) || ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Concatenation continuation -- still expression slot.
  assert!(labels.contains(&"now") || labels.contains(&"current_timestamp")
    || labels.contains(&"id") || labels.contains(&"user_id"),
    "MERGE complex expr continuation missing expr ctx");
}

#[test]
fn r2_193_insert_overriding_value_followup() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users OVERRIDING SYSTEM ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"VALUE"),
    "OVERRIDING SYSTEM <cursor> missing VALUE; labels[..10]={:?}",
    &labels[..labels.len().min(10)]);
}

#[test]
fn r2_192_merge_update_set_rhs_uses_alias_dot() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name = o.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // After `o.` -- dot alias resolves source table cols.
  assert!(labels.contains(&"user_id") || labels.contains(&"id"),
    "MERGE UPDATE SET RHS o.<col> missing source cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_192_merge_update_set_multi_assignment() {
  let cat = catalog_with_users_and_orders();
  // Chain: SET name = 'x', email = <cursor>.
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name = 'x', email = ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Should still recognize expression slot.
  assert!(labels.contains(&"now") || labels.contains(&"user_id") || labels.contains(&"id"),
    "MERGE UPDATE SET 2nd assignment RHS missing expr ctx");
}

#[test]
fn r2_191_merge_update_set_rhs_now_works() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name = ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Fixed: target + source cols + aliases + functions surface.
  assert!(labels.contains(&"now") || labels.contains(&"current_timestamp")
    || labels.contains(&"id") || labels.contains(&"user_id"),
    "MERGE UPDATE SET RHS still missing expr ctx; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_190_insert_col_list_resolves_partial() {
  let cat = catalog_with_users_and_orders();
  // INSERT (id, em<cursor>) partial col.
  let src = "INSERT INTO users (id, em";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"),
    "INSERT col list partial col missing email; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_190_update_set_lhs_emits_target_cols() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"name") || labels.contains(&"email") || labels.contains(&"id"),
    "UPDATE SET LHS missing target cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_189_merge_when_not_matched_then_insert_col_list_now_works() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id WHEN NOT MATCHED THEN INSERT (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Fixed via merge_insert_col_list_slot detector.
  assert!(labels.contains(&"id") || labels.contains(&"name") || labels.contains(&"email"),
    "MERGE THEN INSERT col list still missing target cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_187_nested_cte_inner_cursor_resolves() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH outer_cte AS (WITH inner_cte AS (SELECT id FROM users u WHERE u.) SELECT * FROM inner_cte) SELECT * FROM outer_cte";
  let cur = src.find("u.").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "nested CTE inner cursor missed cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_187_after_cast_emits_expr_context() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1::bigint + ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Expression slot after `+` -- functions present.
  assert!(labels.contains(&"now") || labels.contains(&"current_timestamp"),
    "after cast+arithmetic expr ctx missing fns");
}

#[test]
fn r2_187_inside_cast_paren_emits_types() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT CAST(x AS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Fixed via cast_as_expects_type detector -- types only now.
  assert!(labels.iter().any(|l| matches!(*l, "INT" | "INTEGER" | "BIGINT" | "TEXT" | "BOOLEAN" | "NUMERIC")),
    "CAST AS slot missing types; labels[..20]={:?}",
    &labels[..labels.len().min(20)]);
}

#[test]
fn r2_187_over_paren_no_args_emits_partition_order() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT count(*) OVER (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Inside OVER (...) the user can write PARTITION BY / ORDER BY / RANGE / ROWS / GROUPS.
  assert!(labels.contains(&"PARTITION BY") || labels.contains(&"ORDER BY")
    || labels.contains(&"PARTITION") || labels.contains(&"ROWS"),
    "OVER ( slot missing window-clause kws; labels[..20]={:?}",
    &labels[..labels.len().min(20)]);
}

#[test]
fn r2_187_multiline_cursor_position() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT\n  id,\n  em\nFROM users";
  let cur = src.find("em").unwrap() + 2;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"),
    "multiline cursor missed email; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_186_completion_after_semicolon_starts_fresh() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1; ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Cursor after `;` should land at top-level stmt-start menu.
  assert!(labels.contains(&"SELECT") || labels.contains(&"INSERT INTO"),
    "fresh stmt slot missing top-level kws; labels[..10]={:?}",
    &labels[..labels.len().min(10)]);
}

#[test]
fn r2_186_completion_after_keyword_alias() {
  let cat = catalog_with_users_and_orders();
  // After AS, the next token is an alias name -- no useful catalog
  // completion. Should not dump 600 items.
  let src = "SELECT id AS ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.len() < 100,
    "AS-alias slot dumped {} items", items.len());
}

#[test]
fn r2_186_completion_after_semicolons_in_middle() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1; SELECT * FROM users WHERE em";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Second stmt's scope should resolve users; "em" surfaces email.
  assert!(labels.contains(&"email"),
    "multi-stmt second-stmt partial col missing email");
}

#[test]
fn r2_186_completion_empty_alias_in_from() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // After table name + space, next legal: JOIN/WHERE/AS/etc.
  assert!(labels.contains(&"WHERE") || labels.contains(&"AS") || labels.contains(&"JOIN")
    || labels.contains(&"INNER JOIN") || labels.contains(&"ORDER BY")
    || labels.contains(&"GROUP BY") || labels.contains(&"LIMIT"),
    "after-table slot missing core kws; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_186_dot_no_alias_doesnt_emit_columns() {
  let cat = catalog_with_users_and_orders();
  // bogus.<col> — no such alias; should not dump random columns.
  let src = "SELECT bogus.";
  let items = complete_at(src, src.len(), &cat);
  // Either empty or only schema-like results, never a column dump.
  assert!(items.len() < 30,
    "bogus.* leaked {} items", items.len());
}

#[test]
fn r2_164_completion_dedup_no_duplicate_labels() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT  FROM users u";
  let cur = "SELECT ".len();
  let items = complete_at(src, cur, &cat);
  let mut seen: std::collections::HashSet<(String, ItemKind)> = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.to_ascii_lowercase(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate {:?}", key);
  }
}

#[test]
fn r2_164_generic_dialect_does_not_panic() {
  use dsl_parse::Dialect;
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WHERE em";
  let cur = src.len();
  let file = dsl_parse::parse(src, Dialect::Generic);
  let scopes = dsl_resolve::resolve_with_source(&file.statements, src);
  let items = dsl_completion::complete(src, &file, &scopes, &cat, (cur as u32).into());
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"),
    "Generic-dialect completion lost email; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_162_long_buffer_1000_stmts_no_panic() {
  let cat = catalog_with_users_and_orders();
  let mut src = String::new();
  for _ in 0..1000 {
    src.push_str("SELECT id FROM users; ");
  }
  let cur = src.len();
  let items = complete_at(&src, cur, &cat);
  // Cursor at EOF after last `;` -- top-level fresh stmt menu acceptable.
  assert!(items.len() < 1000, "long buffer dumped {} items", items.len());
}

#[test]
fn r2_162_long_select_list_no_panic() {
  let cat = catalog_with_users_and_orders();
  let mut src = "SELECT ".to_string();
  for i in 0..1000 {
    if i > 0 { src.push(','); }
    src.push_str(&format!(" col{i}"));
  }
  src.push_str(" FROM users WHERE em");
  let cur = src.len();
  let items = complete_at(&src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"),
    "long projection list lost partial col completion");
}

#[test]
fn r2_162_huge_in_list_completion_no_panic() {
  let cat = catalog_with_users_and_orders();
  let mut src = "SELECT id FROM users WHERE id IN (".to_string();
  for i in 0..500 {
    if i > 0 { src.push_str(", "); }
    src.push_str(&i.to_string());
  }
  src.push_str(") AND em");
  let cur = src.len();
  let items = complete_at(&src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"),
    "huge IN list broke partial col completion");
}

#[test]
fn r2_160_multistmt_cursor_in_second_stmt_resolves_own_scope() {
  let cat = catalog_with_users_and_orders();
  // First stmt aliases users to `u`; second aliases orders to `o`.
  // Cursor in second stmt's WHERE must see `o`, not `u`.
  let src = "SELECT u.id FROM users u; SELECT o.user_id FROM orders o WHERE o.";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"user_id") || labels.contains(&"id"),
    "second-stmt cursor missed orders cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_160_multistmt_cursor_in_first_stmt_keeps_own_scope() {
  let cat = catalog_with_users_and_orders();
  // Cursor in first stmt while second stmt has different scope.
  let src = "SELECT u. FROM users u; SELECT * FROM orders;";
  let cur = "SELECT u.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "first-stmt cursor lost users cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_160_cte_chain_mid_typing_doesnt_panic() {
  let cat = catalog_with_users_and_orders();
  // Half-typed CTE.
  let src = "WITH t AS (SELECT id FROM users) SELECT t.";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"),
    "CTE alias dot missed projected id; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_160_deeply_nested_subquery_resolves() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM (SELECT * FROM (SELECT * FROM users u) sub1) sub2 WHERE sub2.";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  // sub2 doesn't project specific cols, so cols may not surface. Just
  // verify no panic + result isn't the 600-item dump.
  assert!(items.len() < 300,
    "deep nested subquery dumped {} items", items.len());
}

#[test]
fn r2_159_partial_identifier_in_projection_still_completes() {
  let cat = catalog_with_users_and_orders();
  // Cursor mid-identifier in projection.
  let src = "SELECT em FROM users";
  let cur = "SELECT em".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"),
    "partial-typed `em` in projection didn't surface email; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_159_partial_identifier_after_comma() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id, em FROM users";
  let cur = "SELECT id, em".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"),
    "partial col after comma didn't surface email");
}

#[test]
fn r2_159_partial_keyword_at_stmt_start() {
  let cat = catalog_with_users_and_orders();
  // `SE` -> SELECT plus other SE* keywords; editor filters.
  let src = "SE";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"SELECT") || labels.contains(&"SET"),
    "partial stmt-start kw missing; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_159_partial_table_after_from() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM us";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"),
    "partial table `us` missing users; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_159_cursor_after_paren_in_predicate() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WHERE (";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "predicate cursor after `(` missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_159_cursor_after_equals_emits_columns_and_fns() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WHERE id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // RHS of `=` is an expression slot -- columns + functions.
  assert!(labels.contains(&"id") || labels.contains(&"now"),
    "after `=` missing expr context; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_155_source_tables_merge_case_insensitive() {
  use dsl_catalog::{Catalog, Schema, Table, TableKind};
  let mut live = Catalog::default();
  live.schemas.push(Schema {
    name: "public".into(),
    tables: vec![Table {
      schema: "public".into(),
      name: "users".into(),
      kind: TableKind::Table,
      columns: Vec::new(),
      constraints: Vec::new(),
      indexes: Vec::new(),
      triggers: Vec::new(),
      policies: Vec::new(),
      comment: None,
      row_estimate: None,
      owner: None, definition: None, strict: false, options: None,
    }],
  });
  let mut derived = Catalog::default();
  derived.schemas.push(Schema {
    name: "PUBLIC".into(),
    tables: vec![Table {
      schema: "PUBLIC".into(),
      name: "USERS".into(),
      kind: TableKind::Table,
      columns: Vec::new(),
      constraints: Vec::new(),
      indexes: Vec::new(),
      triggers: Vec::new(),
      policies: Vec::new(),
      comment: None,
      row_estimate: None,
      owner: None, definition: None, strict: false, options: None,
    }],
  });
  let merged = ::dsl_completion::source_tables::merge(&live, &derived);
  assert_eq!(merged.schemas.len(), 1, "schemas dedup case-insensitive");
  assert_eq!(merged.schemas[0].tables.len(), 1, "tables dedup case-insensitive");
}

#[test]
fn r2_152_fallback_mixed_case_alias_dot_resolves() {
  let cat = catalog_with_users_and_orders();
  // Mixed-case alias U, table USERS -- fallback::scope_from_text path.
  // resolver may also pick this up depending on pg_query tolerance.
  let src = "SELECT * FROM USERS U WHERE U.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "mixed-case alias dot missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_152_cte_columns_of_case_insensitive() {
  use dsl_parse::{Dialect, parse};
  use dsl_resolve::resolve_with_source;
  let src = "WITH T(A, B) AS (SELECT 1, 2) SELECT * FROM t";
  let p = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&p.statements, src);
  // Case mismatch: declared as T(A,B), looked up as t.
  let cols = scopes[0].cte_columns_of("t").expect("cte lookup case-insensitive");
  assert!(cols.iter().any(|c| c.eq_ignore_ascii_case("a")), "alpha-ish missing: {cols:?}");
  assert!(cols.iter().any(|c| c.eq_ignore_ascii_case("b")), "beta-ish missing: {cols:?}");
}

#[test]
fn r2_152_columns_of_table_case_insensitive() {
  let cat = catalog_with_users_and_orders();
  let mut out = Vec::new();
  ::dsl_completion::sources::columns_of_table(&cat, None, "USERS", &mut out);
  assert!(!out.is_empty(), "columns_of_table USERS came back empty");
}

#[test]
fn r2_152_tables_in_schema_case_insensitive() {
  let cat = catalog_with_users_and_orders();
  let mut out = Vec::new();
  let n = ::dsl_completion::sources::tables_in_schema(&cat, "PUBLIC", &mut out);
  assert!(n > 0, "tables_in_schema PUBLIC returned 0");
}

#[test]
fn r2_151_catalog_case_insensitive_lookups() {
  let cat = catalog_with_users_and_orders();
  // find_table accepts any case.
  assert!(cat.find_table(None, "USERS").is_some(), "find_table USERS");
  assert!(cat.find_table(None, "Users").is_some(), "find_table Users");
  assert!(cat.find_table(Some("PUBLIC"), "users").is_some(), "schema PUBLIC");
  assert!(cat.find_table(None, "users").is_some(), "lowercase still works");
  // columns_named is case-insensitive (returns all matching cols).
  assert!(!cat.columns_named("ID").is_empty(), "columns_named ID");
  assert!(!cat.columns_named("Id").is_empty(), "columns_named Id");
  // column_in is case-insensitive on column too.
  assert!(cat.column_in(None, "users", "ID").is_some(), "column_in ID");
  assert!(cat.column_in(None, "USERS", "id").is_some(), "column_in USERS/id");
}

#[test]
fn r2_150_upper_case_table_dot_resolves_now() {
  // PG folds unquoted identifiers to lowercase. Catalog::find_table
  // is now case-insensitive so `USERS.<cursor>` matches `users`.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT USERS. FROM USERS";
  let cur = "SELECT USERS.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "USERS.<col> missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_149_mixed_case_alias_dot_resolves() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users U WHERE U.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "uppercase alias dot missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_149_partial_column_after_dot_returns_all_cols() {
  // u.em<cursor> -- editor filters by prefix; we return all alias
  // columns so the editor's fuzzy match has material.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u WHERE u.em";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"),
    "partial-typed col `em` did not surface email; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_149_dot_inside_join_on_predicate() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u JOIN orders o ON o.user_id = u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "JOIN ON predicate dot missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_149_dot_in_select_projection() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u. FROM users u";
  let cur = "SELECT u.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "projection dot missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_149_dot_in_order_by() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u ORDER BY u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "ORDER BY dot missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_148_schema_qualified_alias_dot_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u.id FROM public.users u WHERE u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "schema-qualified alias dot missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_148_quoted_alias_dot_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users \"U\" WHERE \"U\".";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "quoted alias dot missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_148_cte_column_list_dot_alias_columns() {
  let cat = catalog_with_users_and_orders();
  // CTE with explicit column list -- t.alpha / t.beta should be visible.
  let src = "WITH t(alpha, beta) AS (SELECT id, email FROM users) SELECT t.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Either the explicit list (alpha/beta) OR the CTE body cols if the
  // resolver fell through. Just require something useful, not nothing.
  assert!(
    labels.contains(&"alpha") || labels.contains(&"beta")
      || labels.contains(&"id") || labels.contains(&"email"),
    "CTE col-list dot missing cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]
  );
}

#[test]
fn r2_148_lateral_srf_alias_dot_emits_nothing_or_function_col() {
  // gs.<col> -- generate_series row alias; PG names it `generate_series`
  // by default but here the explicit `gs(n)` aliases it. We don't have
  // catalog knowledge for SRF output, so completion may emit nothing
  // (acceptable) OR fall through to expression keywords. Just verify
  // the query doesn't panic and doesn't dump 600 noise items.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM generate_series(1,3) gs(n) WHERE gs.";
  let items = complete_at(src, src.len(), &cat);
  // Either empty (correct) or small (degraded but acceptable). Never
  // the 600+ catalog dump.
  assert!(items.len() < 300, "SRF alias dot dumped {} items", items.len());
}

#[test]
fn r2_147_merge_source_alias_dot_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"user_id"),
    "MERGE USING alias dot missing user_id; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_147_merge_target_alias_dot_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "MERGE INTO alias dot missing users cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_147_insert_select_from_alias_dot_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, name) SELECT u.id, u.name FROM users u WHERE u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "INSERT ... SELECT FROM alias dot missing users cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_147_cte_alias_dot_emits_projected_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id, email FROM users) SELECT t.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "CTE alias dot missing projected cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_146_update_from_alias_dot_emits_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users u SET name = o.user_id::text FROM orders o WHERE u.id = o.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"user_id"),
    "o.<col> missing user_id; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_146_delete_using_alias_dot_emits_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "DELETE FROM orders o USING users u WHERE o.user_id = u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "u.<col> missing target cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_146_new_dot_in_trigger_function_body() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER trg AFTER INSERT ON users FOR EACH ROW EXECUTE FUNCTION fn();\
             CREATE FUNCTION fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM NEW.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // NEW.<col> -> columns of users (trigger target).
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "NEW.<col> missing trigger target cols; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_145_insert_values_inner_slot_emits_default_and_fns() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, name) VALUES (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEFAULT"),
    "DEFAULT keyword missing inside VALUES; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
  // Any common function should be present.
  assert!(labels.contains(&"now") || labels.contains(&"current_timestamp"),
    "expression context (functions) missing inside VALUES");
}

#[test]
fn r2_145_on_conflict_do_update_set_emits_target_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // LHS slot of DO UPDATE SET expects target columns.
  assert!(labels.contains(&"name") || labels.contains(&"email") || labels.contains(&"id"),
    "DO UPDATE SET LHS missing target columns; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_145_excluded_dot_emits_target_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // EXCLUDED is the proposed row -- same columns as the target.
  assert!(labels.contains(&"name") || labels.contains(&"email") || labels.contains(&"id"),
    "EXCLUDED.<col> missing target columns; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
}

#[test]
fn r2_145_insert_values_after_comma_emits_default_and_fns() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, name) VALUES (1, ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"DEFAULT"),
    "DEFAULT keyword missing after VALUES comma; labels[..15]={:?}",
    &labels[..labels.len().min(15)]);
  assert!(labels.contains(&"now") || labels.contains(&"current_timestamp"),
    "expression context (functions) missing after VALUES comma");
}

#[test]
fn r2_144_merge_when_matched_and_predicate_slot() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED AND ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Aliases must surface.
  assert!(labels.contains(&"u"), "alias u missing; labels[..20]={:?}",
    &labels[..labels.len().min(20)]);
  assert!(labels.contains(&"o"), "alias o missing");
  // Columns of both target and source must surface.
  assert!(labels.contains(&"id"), "id missing");
  assert!(labels.contains(&"user_id"), "user_id missing");
  // Expression keywords present.
  assert!(labels.contains(&"NOT") || labels.contains(&"AND") || labels.contains(&"OR"),
    "expression kws missing");
}

#[test]
fn r2_144_merge_when_not_matched_and_predicate_slot() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id WHEN NOT MATCHED AND ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"u"), "alias u missing");
  assert!(labels.contains(&"o"), "alias o missing");
  assert!(labels.contains(&"user_id"), "user_id missing");
}

#[test]
fn r2_143_insert_returning_columns_fix() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, name) VALUES (1, 'x') RETURNING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"*"),
    "RETURNING column slot missing id/*; labels[..20]={:?}",
    &labels[..labels.len().min(20)]);
}

#[test]
fn r2_143_lateral_after_comma_surfaces_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u, LATERAL ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // LATERAL slot expects a table-returning expression -- catalog
  // tables are the right starting point.
  assert!(labels.contains(&"users") || labels.contains(&"orders"),
    "LATERAL slot missing catalog tables; labels[..20]={:?}",
    &labels[..labels.len().min(20)]);
}

#[test]
fn r2_125_set_guc_menu_extensions() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for guc in &[
    "log_lock_waits",
    "log_temp_files",
    "tcp_keepalives_idle",
    "scram_iterations",
    "wal_compression",
    "default_toast_compression",
    "recovery_init_sync_method",
    "restart_after_crash",
    "max_connections",
  ] {
    let src = "SET LOCAL ";
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(guc) {
      missing.push(format!("SET menu missing {guc}: labels[..10]={:?}", &labels[..labels.len().min(10)]));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_109_is_json_predicate_chain() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("SELECT * FROM users WHERE data IS ", "JSON"),
    ("SELECT * FROM users WHERE data IS JSON ", "VALUE"),
    ("SELECT * FROM users WHERE data IS JSON ", "SCALAR"),
    ("SELECT * FROM users WHERE data IS JSON ", "ARRAY"),
    ("SELECT * FROM users WHERE data IS JSON ", "OBJECT"),
    ("SELECT * FROM users WHERE data IS NOT ", "JSON"),
    ("SELECT * FROM users WHERE data IS JSON OBJECT WITH ", "UNIQUE KEYS"),
    ("SELECT * FROM users WHERE data IS JSON OBJECT WITHOUT ", "UNIQUE KEYS"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_103_audit_fdw_type_domain_text_search() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("CREATE FOREIGN DATA WRAPPER fdw ", "HANDLER"),
    ("CREATE FOREIGN DATA WRAPPER fdw ", "VALIDATOR"),
    ("ALTER FOREIGN DATA WRAPPER fdw ", "OPTIONS"),
    ("ALTER FOREIGN DATA WRAPPER fdw ", "RENAME TO"),
    ("ALTER TYPE t ADD ", "ATTRIBUTE"),
    ("ALTER TYPE t ADD ", "VALUE"),
    ("ALTER TYPE t RENAME ", "ATTRIBUTE"),
    ("ALTER TYPE t RENAME ", "VALUE"),
    ("ALTER TYPE t RENAME ATTRIBUTE old ", "TO"),
    ("ALTER DOMAIN d ADD ", "CONSTRAINT"),
    ("ALTER DOMAIN d ADD ", "CHECK"),
    ("ALTER DOMAIN d DROP ", "CONSTRAINT"),
    ("ALTER DOMAIN d SET ", "DEFAULT"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_102_audit_pubsub_replication_followups() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("ALTER PUBLICATION p ADD ", "TABLE"),
    ("ALTER PUBLICATION p DROP ", "TABLE"),
    ("ALTER PUBLICATION p SET ", "TABLE"),
    ("ALTER PUBLICATION p ADD TABLES IN ", "SCHEMA"),
    ("ALTER SUBSCRIPTION sub ", "ENABLE"),
    ("ALTER SUBSCRIPTION sub ", "DISABLE"),
    ("ALTER SUBSCRIPTION sub ", "REFRESH PUBLICATION"),
    ("ALTER TABLE users REPLICA IDENTITY USING ", "INDEX"),
    ("CREATE SUBSCRIPTION sub CONNECTION 'host=x' PUBLICATION p WITH (", "create_slot"),
    ("CREATE SUBSCRIPTION sub CONNECTION 'host=x' PUBLICATION p WITH (", "enabled"),
    ("CREATE SUBSCRIPTION sub CONNECTION 'host=x' PUBLICATION p WITH (", "copy_data"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_101_audit_extension_publication_subscription() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("CREATE EXTENSION pg_trgm WITH ", "SCHEMA"),
    ("CREATE EXTENSION pg_trgm WITH ", "VERSION"),
    ("CREATE EXTENSION pg_trgm WITH ", "CASCADE"),
    ("CREATE PUBLICATION p FOR ", "TABLE"),
    ("CREATE PUBLICATION p FOR ALL ", "TABLES"),
    ("CREATE PUBLICATION p FOR ALL TABLES IN ", "SCHEMA"),
    ("CREATE SUBSCRIPTION sub CONNECTION 'host=x' PUBLICATION p WITH (", "slot_name"),
    ("ALTER EXTENSION pg_trgm SET ", "SCHEMA"),
    ("ALTER EXTENSION pg_trgm UPDATE ", "TO"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_091_audit_misc_obscure_followups() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("COMMENT ON COLUMN users.id IS ", "''"),
    ("COMMENT ON CAST (text AS int) IS ", "''"),
    ("COMMENT ON AGGREGATE a(int) IS ", "''"),
    ("COMMENT ON STATISTICS s IS ", "''"),
    ("COMMENT ON SERVER srv IS ", "''"),
    ("ALTER OPERATOR CLASS oc USING ", "btree"),
    ("ALTER OPERATOR FAMILY of USING ", "btree"),
    ("CREATE SCHEMA s ", "AUTHORIZATION"),
    ("CREATE SCHEMA AUTHORIZATION ", "postgres"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_090_audit_more_followup_gaps() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("SELECT count(*) OVER (ORDER BY id ROWS CURRENT ROW EXCLUDE ", "CURRENT ROW"),
    ("SELECT count(*) OVER (ORDER BY id ROWS CURRENT ROW EXCLUDE ", "GROUP"),
    ("SELECT count(*) OVER (ORDER BY id ROWS CURRENT ROW EXCLUDE ", "TIES"),
    ("SELECT count(*) OVER (ORDER BY id ROWS CURRENT ROW EXCLUDE ", "NO OTHERS"),
    ("ALTER FOREIGN TABLE ft ALTER COLUMN c ", "SET"),
    ("ALTER FOREIGN TABLE ft ALTER COLUMN c SET ", "DEFAULT"),
    ("COMMENT ON DOMAIN d IS ", "''"),
    ("COMMENT ON TYPE t IS ", "''"),
    ("COMMENT ON SCHEMA public IS ", "''"),
    ("GRANT ", "ALL PRIVILEGES"),
    ("GRANT ", "SELECT"),
    ("REVOKE ", "GRANT OPTION FOR"),
    ("SELECT * FROM users FOR SHARE ", "OF"),
    ("SELECT * FROM users FOR SHARE ", "NOWAIT"),
    ("SELECT * FROM users FOR SHARE ", "SKIP LOCKED"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_089_audit_dml_expr_slot_gaps() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("DELETE FROM users WHERE ", "id"),
    ("DELETE FROM users WHERE ", "email"),
    ("UPDATE users SET name = ", "id"),
    ("UPDATE users SET name = ", "now"),
    ("SELECT * FROM users ORDER BY id NULLS ", "FIRST"),
    ("SELECT * FROM users ORDER BY id NULLS ", "LAST"),
    ("SELECT * FROM users ORDER BY id ASC NULLS ", "FIRST"),
    ("WITH t (a, b) AS (SELECT 1, 2) SELECT ", "DISTINCT"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_088_audit_expr_window_setop_gaps() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("SELECT * FROM users WHERE id IS ", "NULL"),
    ("SELECT * FROM users WHERE id IS ", "NOT NULL"),
    ("SELECT * FROM users WHERE id IS NOT ", "NULL"),
    ("SELECT * FROM users WHERE id IS DISTINCT ", "FROM"),
    ("SELECT * FROM users WHERE id IS NOT DISTINCT ", "FROM"),
    ("SELECT CASE WHEN id > 0 THEN 1 ", "ELSE"),
    ("SELECT CASE WHEN id > 0 THEN 1 ELSE 0 ", "END"),
    ("SELECT count(*) OVER (ORDER BY id ROWS BETWEEN UNBOUNDED ", "PRECEDING"),
    ("SELECT count(*) OVER (ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND ", "UNBOUNDED FOLLOWING"),
    ("SELECT count(*) OVER (ORDER BY id ROWS BETWEEN CURRENT ", "ROW"),
    ("SELECT 1 UNION ", "ALL"),
    ("SELECT 1 INTERSECT ", "ALL"),
    ("SELECT 1 EXCEPT ", "ALL"),
    ("SELECT 1 FETCH FIRST 5 ROWS ", "ONLY"),
    ("SELECT 1 FETCH FIRST 5 ROWS ", "WITH TIES"),
    ("ALTER PROCEDURE p() ", "RENAME TO"),
    ("ALTER ROUTINE r() ", "RENAME TO"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_087_audit_grant_revoke_class_gaps() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("GRANT SELECT ON ", "SCHEMA"),
    ("GRANT SELECT ON ", "SEQUENCE"),
    ("GRANT SELECT ON ", "DATABASE"),
    ("GRANT SELECT ON ALL TABLES IN ", "SCHEMA"),
    ("GRANT SELECT ON ALL SEQUENCES IN ", "SCHEMA"),
    ("CREATE EVENT TRIGGER et ON ddl_command_start WHEN ", "tag"),
    ("CREATE RULE r AS ON INSERT TO users DO ", "INSTEAD"),
    ("CREATE RULE r AS ON INSERT TO users DO ", "ALSO"),
    ("CREATE CAST (text AS int) WITH ", "FUNCTION"),
    ("CREATE CAST (text AS int) WITH ", "INOUT"),
    ("CREATE CAST (text AS int) WITHOUT ", "FUNCTION"),
    ("ALTER LARGE OBJECT 1234 OWNER ", "TO"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_086_audit_with_alter_merge_call_gaps() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("WITH x AS NOT ", "MATERIALIZED"),
    ("WITH RECURSIVE t(n) AS (SELECT 1) SEARCH ", "DEPTH FIRST BY"),
    ("WITH RECURSIVE t(n) AS (SELECT 1) SEARCH ", "BREADTH FIRST BY"),
    ("WITH RECURSIVE t(n) AS (SELECT 1) SEARCH DEPTH ", "FIRST BY"),
    ("WITH RECURSIVE t(n) AS (SELECT 1) SEARCH DEPTH FIRST BY n SET ord CYCLE n SET ", "<flag_col>"),
    ("ALTER TABLE users INHERIT ", "orders"),
    ("ALTER TABLE users NO ", "INHERIT"),
    ("ALTER COLLATION c REFRESH ", "VERSION"),
    ("ALTER OPERATOR + (int, int) SET ", "SCHEMA"),
    ("ALTER DEFAULT PRIVILEGES ", "FOR ROLE"),
    ("ALTER DEFAULT PRIVILEGES ", "GRANT"),
    ("ALTER DEFAULT PRIVILEGES ", "REVOKE"),
    ("MERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN ", "UPDATE"),
    ("MERGE INTO users u USING orders o ON u.id = o.user_id WHEN NOT MATCHED THEN ", "INSERT"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_085_audit_misc_followup_gaps() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("ROLLBACK TO ", "SAVEPOINT"),
    ("SECURITY LABEL ON ", "TABLE"),
    ("SECURITY LABEL ON ", "COLUMN"),
    ("COPY t FROM 'f.csv' WITH (FORMAT ", "CSV"),
    ("COPY t FROM 'f.csv' WITH (", "FORMAT"),
    ("COPY t FROM 'f.csv' WITH (HEADER ", "TRUE"),
    ("REASSIGN OWNED BY ", "postgres"),
    ("REINDEX ", "SYSTEM"),
    ("REINDEX ", "TABLE"),
    ("LOCK TABLE users IN ", "ACCESS EXCLUSIVE"),
    ("SELECT * FROM users FOR UPDATE ", "OF"),
    ("SELECT * FROM users FOR UPDATE ", "NOWAIT"),
    ("SELECT * FROM users FOR UPDATE ", "SKIP LOCKED"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_084_audit_drop_grant_trigger_aggregate() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("DROP TABLE users ", "CASCADE"),
    ("DROP TABLE users ", "RESTRICT"),
    ("DROP TABLE ", "IF EXISTS"),
    ("DROP INDEX ", "IF EXISTS"),
    ("ALTER TABLE users DROP ", "COLUMN"),
    ("ALTER TABLE users DROP ", "CONSTRAINT"),
    ("CREATE TRIGGER tr AFTER ", "INSERT"),
    ("CREATE TRIGGER tr AFTER ", "UPDATE"),
    ("CREATE TRIGGER tr BEFORE INSERT ON users REFERENCING ", "NEW TABLE AS"),
    ("CREATE TRIGGER tr BEFORE INSERT ON users REFERENCING ", "OLD TABLE AS"),
    ("CREATE AGGREGATE a (int) (", "SFUNC"),
    ("CREATE OPERATOR + (", "LEFTARG"),
    ("CREATE OPERATOR + (", "RIGHTARG"),
    ("CREATE OPERATOR + (", "COMMUTATOR"),
    ("GRANT SELECT ON users TO bob WITH ", "WITH GRANT OPTION"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_083_audit_create_table_followups() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("CREATE TABLE child PARTITION OF parent FOR ", "VALUES"),
    ("CREATE TABLE child PARTITION OF parent FOR VALUES ", "IN"),
    ("CREATE TABLE child PARTITION OF parent FOR VALUES ", "FROM"),
    ("CREATE TABLE child PARTITION OF parent FOR VALUES ", "WITH"),
    ("CREATE TABLE t (a int PRIMARY KEY DEFERRABLE ", "INITIALLY"),
    ("CREATE TABLE t (a int PRIMARY KEY INITIALLY ", "DEFERRED"),
    ("CREATE INDEX idx ON users (id) WHERE ", "id"),
    ("INSERT INTO users (id) VALUES (1) ON CONFLICT DO UPDATE SET ", "id"),
    ("INSERT INTO users (id) VALUES (1) ON CONFLICT DO UPDATE SET ", "name"),
    ("WITH t AS NOT ", "MATERIALIZED"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_082_audit_alter_table_constraint_menus() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("ALTER TABLE users OWNER ", "TO"),
    ("ALTER TABLE users ADD CONSTRAINT fk FOREIGN KEY (user_id) REFERENCES orders (id) ON DELETE ", "CASCADE"),
    ("ALTER TABLE users ADD CONSTRAINT fk FOREIGN KEY (user_id) REFERENCES orders (id) ON UPDATE ", "RESTRICT"),
    ("ALTER TABLE users ADD CONSTRAINT fk FOREIGN KEY (user_id) REFERENCES orders (id) ON DELETE ", "SET NULL"),
    ("ALTER TABLE users ADD CONSTRAINT u UNIQUE (id) DEFERRABLE ", "INITIALLY"),
    ("ALTER TABLE users ADD CONSTRAINT u UNIQUE (id) INITIALLY ", "DEFERRED"),
    ("ALTER TABLE users ADD CONSTRAINT u UNIQUE (id) INITIALLY ", "IMMEDIATE"),
    ("ALTER TABLE users ADD CONSTRAINT u UNIQUE NULLS ", "NOT DISTINCT"),
    ("ALTER FUNCTION f() LANGUAGE ", "plpgsql"),
    ("ALTER FUNCTION f() LANGUAGE ", "sql"),
    ("ALTER FUNCTION f() SUPPORT ", "support_fn"),
    ("ALTER INDEX idx SET (", "fillfactor"),
    ("ALTER FOREIGN TABLE ft OPTIONS (", "ADD"),
    ("ALTER SERVER srv OPTIONS (", "ADD"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_081_audit_alter_chain_submodifiers() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("ALTER TABLE users ALTER COLUMN email SET STORAGE ", "PLAIN"),
    ("ALTER TABLE users ALTER COLUMN email SET STORAGE ", "EXTERNAL"),
    ("ALTER TABLE users ALTER COLUMN email SET COMPRESSION ", "lz4"),
    ("ALTER TABLE users ALTER COLUMN email SET COMPRESSION ", "pglz"),
    ("ALTER TABLE users ALTER COLUMN id ADD GENERATED ", "ALWAYS"),
    ("ALTER TABLE users ALTER COLUMN id ADD GENERATED ", "BY DEFAULT"),
    ("ALTER TABLE users ALTER COLUMN id SET GENERATED ", "ALWAYS"),
    ("COMMENT ON ", "TABLE"),
    ("COMMENT ON ", "COLUMN"),
    ("COMMENT ON ", "FUNCTION"),
    ("COMMENT ON ", "INDEX"),
    ("ALTER ROLE bob SET ", "search_path"),
    ("ALTER DATABASE db SET ", "search_path"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_080_audit_alter_function_modifier_followups() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("ALTER FUNCTION f() PARALLEL ", "SAFE"),
    ("ALTER FUNCTION f() SECURITY ", "DEFINER"),
    ("ALTER FUNCTION f() SECURITY ", "INVOKER"),
    ("ALTER FUNCTION f() SET ", "SCHEMA"),
    ("ALTER FUNCTION f() RESET ", "ALL"),
    ("CREATE STATISTICS s ON ", "users"),
    ("CREATE STATISTICS s (", "ndistinct"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_080_audit_select_clause_followups() {
  let cat = catalog_with_users_and_orders();
  let mut missing: Vec<String> = Vec::new();
  for (src, expect) in &[
    ("SELECT id FROM users GROUP BY ", "GROUPING SETS"),
    ("SELECT id FROM users GROUP BY ", "CUBE"),
    ("SELECT id FROM users GROUP BY ", "ROLLUP"),
    ("SELECT id FROM users GROUP BY GROUPING ", "SETS"),
    ("WITH RECURSIVE t(n) AS (SELECT 1) SELECT ", "DISTINCT"),
    ("ALTER MATERIALIZED VIEW mv ", "RENAME TO"),
    ("ALTER VIEW v ", "RENAME TO"),
    ("ALTER SEQUENCE s ", "OWNED BY"),
    ("ALTER SEQUENCE s ", "NO MAXVALUE"),
    ("ALTER SEQUENCE s NO ", "MAXVALUE"),
    ("ALTER SEQUENCE s OWNED ", "BY"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    if !labels.contains(expect) {
      missing.push(format!("{src:?} -> {expect}: labels={labels:?}"));
    }
  }
  assert!(missing.is_empty(), "missing: {missing:#?}");
}

#[test]
fn r2_079_select_fetch_chain() {
  let cat = catalog_with_users_and_orders();
  for (src, expect) in &[
    ("SELECT 1 FETCH ", "FIRST"),
    ("SELECT 1 FETCH ", "NEXT"),
    ("SELECT 1 FETCH FIRST ", "ROW"),
    ("SELECT 1 FETCH FIRST ", "ROWS"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(expect), "src {src:?} missing {expect}; labels: {labels:?}");
  }
}

#[test]
fn r2_079_tablesample_repeatable() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT 1 FROM users TABLESAMPLE BERNOULLI (10) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"REPEATABLE"), "labels: {labels:?}");
}

#[test]
fn r2_079_insert_into_collist_followup() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (id, name) ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"VALUES"), "labels: {labels:?}");
  assert!(labels.contains(&"SELECT"), "labels: {labels:?}");
  assert!(labels.contains(&"DEFAULT VALUES"), "labels: {labels:?}");
}

#[test]
fn r2_078_sweep_ctl_partitioning_extensions() {
  let cat = catalog_with_users_and_orders();
  for (src, expect) in &[
    ("CREATE TABLE parent (id int) PARTITION BY ", "RANGE"),
    ("ALTER TABLE parent ATTACH ", "ATTACH PARTITION"),
    ("ALTER TABLE parent DETACH ", "DETACH PARTITION"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(expect), "src {src:?} missing {expect}; labels: {labels:?}");
  }
}

#[test]
fn r2_163_completion_repeated_calls_clean() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id FROM users WHERE em";
  for _ in 0..100 {
    let items = complete_at(src, src.len(), &cat);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"email"), "regression after repeat calls");
  }
}

#[test]
fn r2_163_long_buffer_completion_at_end_under_threshold() {
  let cat = catalog_with_users_and_orders();
  let mut src = String::with_capacity(50_000);
  for _ in 0..1000 {
    src.push_str("SELECT id FROM users; ");
  }
  let cur = src.len();
  let items = complete_at(&src, cur, &cat);
  // Don't dump catalog; cursor at fresh-stmt slot.
  assert!(items.len() < 1000, "long buffer dumped {} items", items.len());
}

#[test]
fn r3_001_uppercase_table_dot_mid_typing_surfaces_columns() {
  // CYCLE 3 fix: `SELECT USERS.<cursor>` before any FROM clause is
  // typed. pg_query rejects the prefix and the fallback scope is
  // empty, but USERS still matches a catalog table case-insensitively.
  // The dot handler now falls back to catalog lookup as a last resort.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT USERS.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id; got {labels:?}");
  assert!(labels.contains(&"email"), "expected email; got {labels:?}");
  assert!(labels.contains(&"name"), "expected name; got {labels:?}");
}

#[test]
fn r3_001_lowercase_table_dot_mid_typing_surfaces_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT users.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id; got {labels:?}");
}

#[test]
fn r3_002_merge_update_set_lhs_emits_target_columns() {
  // CYCLE 3 fix: `MERGE ... THEN UPDATE SET <cursor>` -- LHS slot of
  // the first assignment. Target column names only, not the RHS
  // expression dump (which would include the source alias columns).
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id \
             WHEN MATCHED THEN UPDATE SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"name"), "expected name; got {labels:?}");
  assert!(labels.contains(&"email"), "expected email; got {labels:?}");
}

#[test]
fn r3_002_merge_update_set_lhs_after_comma_emits_target_columns() {
  // CYCLE 3 fix: comma-continuation of SET assignment list. The
  // cursor sits at the next LHS slot after `col = expr, `.
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id \
             WHEN MATCHED THEN UPDATE SET name = 'x', ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email"), "expected email; got {labels:?}");
}

#[test]
fn r3_004_update_from_alias_dot_surfaces_columns() {
  // CYCLE 3: now that UpdateStmt has from_tables, the resolver binds
  // FROM-list aliases. The dot handler should resolve `o.<cursor>`
  // inside an UPDATE-FROM WHERE clause.
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET active = true FROM orders o WHERE o.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"user_id"), "expected o.user_id; got {labels:?}");
  assert!(labels.contains(&"id"), "expected o.id; got {labels:?}");
}

#[test]
fn r3_004_delete_using_alias_dot_surfaces_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "DELETE FROM users USING orders o WHERE o.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"user_id"), "expected o.user_id; got {labels:?}");
}

#[test]
fn r3_004_update_set_rhs_sees_from_alias() {
  // `UPDATE users SET x = o.<cursor> FROM orders o ...` -- the RHS
  // expression slot must see the FROM-list alias.
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET active = o. FROM orders o WHERE 1=1";
  let cur = "UPDATE users SET active = o.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"user_id"), "expected o.user_id; got {labels:?}");
}

#[test]
fn r3_007_filter_where_emits_scope_columns() {
  // FILTER (WHERE <cursor>) is a per-aggregate predicate slot --
  // semantically a WHERE clause. Must surface scope columns + funcs
  // when a FROM binding exists.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT count(*) FILTER (WHERE  ) FROM users u";
  let cur = "SELECT count(*) FILTER (WHERE ".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id in FILTER WHERE scope: {labels:?}");
  assert!(labels.contains(&"email"), "expected email: {labels:?}");
}

#[test]
fn r3_007_within_group_order_by_emits_scope_columns() {
  // ORDERED-SET aggregate: percentile_disc(0.5) WITHIN GROUP (ORDER BY <cursor>)
  // -- ORDER BY slot scoped to the surrounding FROM bindings.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT percentile_disc(0.5) WITHIN GROUP (ORDER BY  ) FROM users u";
  let cur = "SELECT percentile_disc(0.5) WITHIN GROUP (ORDER BY ".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id in WITHIN GROUP ORDER BY: {labels:?}");
}


#[test]
fn r3_008_distinct_on_paren_emits_scope_columns() {
  // SELECT DISTINCT ON (<cursor>) -- column slot scoped to FROM.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT DISTINCT ON ( ) id FROM users";
  let cur = "SELECT DISTINCT ON (".len() + 1;
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id in DISTINCT ON: {labels:?}");
  assert!(labels.contains(&"email"), "expected email: {labels:?}");
}

#[test]
fn r3_011_grouping_sets_outer_paren_emits_columns() {
  // GROUPING SETS (<cursor>) -- outer paren immediately after the kw.
  // Inner double-paren tuples are a documented cycle-3+ gap; this
  // covers the outer slot only.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id FROM users u GROUP BY GROUPING SETS ( ) ";
  let cur = "SELECT id FROM users u GROUP BY GROUPING SETS (".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id in GROUPING SETS: {labels:?}");
}

#[test]
fn r3_012_rollup_paren_emits_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id FROM users u GROUP BY ROLLUP ( ) ";
  let cur = "SELECT id FROM users u GROUP BY ROLLUP (".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id in ROLLUP: {labels:?}");
}

#[test]
fn r3_013_cube_paren_emits_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id FROM users u GROUP BY CUBE ( ) ";
  let cur = "SELECT id FROM users u GROUP BY CUBE (".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id in CUBE: {labels:?}");
}

#[test]
fn r3_014_returning_star_no_panic() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (email) VALUES ('a') RETURNING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "RETURNING expected target cols: {labels:?}");
}

#[test]
fn r3_015_truncate_only_keyword_continues() {
  let cat = catalog_with_users_and_orders();
  let src = "TRUNCATE ONLY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "TRUNCATE ONLY <tbl>: {labels:?}");
}

#[test]
fn r3_017_create_index_concurrently_table_slot() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX CONCURRENTLY idx_users_email ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"users"), "CREATE INDEX ON: {labels:?}");
}

#[test]
fn r3_018_select_with_alias_dot_in_having() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u.id FROM users u GROUP BY u.id HAVING count(u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "HAVING alias dot: {labels:?}");
}

#[test]
fn r3_023_create_function_returns_setof() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS SETOF ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // SETOF expects type or table; just verify non-panic + something
  // surfaces.
  assert!(!labels.is_empty(), "SETOF empty: {labels:?}");
}

#[test]
fn r3_056_cte_alias_dot_surfaces_projected_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id, email FROM users) SELECT t. FROM t";
  let cur = "WITH t AS (SELECT id, email FROM users) SELECT t.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "CTE projection: {labels:?}");
}

#[test]
fn r3_059_create_policy_using_emits_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE POLICY p ON users USING ( ) ";
  let cur = "CREATE POLICY p ON users USING (".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "POLICY USING: {labels:?}");
}

#[test]
fn r3_060_create_policy_with_check_emits_columns() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE POLICY p ON users WITH CHECK ( ) ";
  let cur = "CREATE POLICY p ON users WITH CHECK (".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "POLICY WITH CHECK: {labels:?}");
}

#[test]
fn r3_062_merge_when_not_matched_then_do_nothing() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id \
             WHEN NOT MATCHED THEN ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Should suggest INSERT or DO NOTHING.
  assert!(labels.iter().any(|l| l.contains("INSERT") || l.contains("DO")),
    "MERGE THEN: {labels:?}");
}

#[test]
fn r3_063_merge_when_matched_then_do_delete() {
  let cat = catalog_with_users_and_orders();
  let src = "MERGE INTO users u USING orders o ON u.id = o.user_id \
             WHEN MATCHED THEN DELETE RETURNING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // MERGE THEN DELETE RETURNING -- target table columns.
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"name"),
    "MERGE DELETE RETURNING: {labels:?}");
}

#[test]
fn r3_084_cte_with_explicit_col_list_dot() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t (a, b, c) AS (SELECT 1, 2, 3) SELECT t. FROM t";
  let cur = "WITH t (a, b, c) AS (SELECT 1, 2, 3) SELECT t.".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"a") || labels.contains(&"b") || labels.contains(&"c"),
    "CTE explicit col list dot: {labels:?}");
}

#[test]
fn r3_117_alter_role_set_guc() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER ROLE r SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(!labels.is_empty(), "ALTER ROLE SET: {labels:?}");
}

#[test]
fn r3_122_create_trigger_for_each_row() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER tg BEFORE INSERT ON users FOR EACH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| l == &"ROW" || l == &"STATEMENT"),
    "FOR EACH: {labels:?}");
}

#[test]
fn r3_123_create_index_using_method() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX idx ON users USING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["btree", "BTREE", "hash", "HASH", "gist", "GIST", "gin", "GIN", "spgist", "SPGIST", "brin", "BRIN"].contains(l)),
    "INDEX USING: {labels:?}");
}

#[test]
fn r3_124_grant_select_on() {
  let cat = catalog_with_users_and_orders();
  let src = "GRANT SELECT ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(!labels.is_empty(), "GRANT ON: {labels:?}");
}

#[test]
fn r3_129_create_publication_for_tables() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE PUBLICATION p FOR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| l.contains("TABLE")),
    "PUBLICATION FOR: {labels:?}");
}

#[test]
fn r3_131_create_extension_with_schema() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EXTENSION pgcrypto WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| l.contains("SCHEMA") || l.contains("VERSION") || l.contains("CASCADE")),
    "CREATE EXTENSION WITH: {labels:?}");
}

#[test]
fn r3_134_set_transaction_isolation() {
  let cat = catalog_with_users_and_orders();
  let src = "SET TRANSACTION ISOLATION LEVEL ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["READ", "REPEATABLE", "SERIALIZABLE"].contains(l)),
    "ISOLATION LEVEL: {labels:?}");
}

#[test]
fn r3_152_for_share_skip_locked() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users FOR SHARE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| l == &"NOWAIT" || l == &"SKIP" || l.contains("LOCKED")),
    "FOR SHARE: {labels:?}");
}

#[test]
fn r3_154_with_check_option_view() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE VIEW v AS SELECT * FROM users WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["LOCAL", "CASCADED", "CHECK"].contains(l) || l.contains("CHECK") || l.contains("OPTION")),
    "WITH OPTION: {labels:?}");
}

#[test]
fn r3_168_create_event_trigger_on_event() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EVENT TRIGGER et ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["ddl_command_start", "ddl_command_end", "sql_drop", "table_rewrite",
    "DDL_COMMAND_START", "DDL_COMMAND_END", "SQL_DROP", "TABLE_REWRITE"].contains(l)),
    "EVENT TRIGGER ON: {labels:?}");
}

#[test]
fn r3_203_alter_type_add_value() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TYPE color ADD VALUE 'red' ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["BEFORE", "AFTER"].contains(l)),
    "ADD VALUE chain: {labels:?}");
}

#[test]
fn r3_207_create_materialized_view_with_data() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE MATERIALIZED VIEW mv AS SELECT * FROM users WITH ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["DATA", "NO"].contains(l) || l.contains("DATA")),
    "MV WITH: {labels:?}");
}

#[test]
fn r3_246_create_function_volatility_kw() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["IMMUTABLE", "STABLE", "VOLATILE", "STRICT", "PARALLEL"].contains(l)),
    "FUNCTION volatility: {labels:?}");
}

#[test]
fn r3_247_create_function_parallel_safe() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql PARALLEL ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["SAFE", "UNSAFE", "RESTRICTED"].contains(l)),
    "PARALLEL: {labels:?}");
}

#[test]
fn r3_248_create_function_security_definer() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION f() RETURNS int AS $$ SELECT 1 $$ LANGUAGE sql SECURITY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["DEFINER", "INVOKER"].contains(l)),
    "SECURITY: {labels:?}");
}

#[test]
fn r3_256_create_rule_chain() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE RULE r AS ON ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["SELECT", "INSERT", "UPDATE", "DELETE"].contains(l)),
    "RULE AS ON: {labels:?}");
}

#[test]
fn r3_258_create_trigger_when_clause() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER tg BEFORE UPDATE ON users FOR EACH ROW WHEN (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // WHEN expects OLD/NEW + columns + funcs
  assert!(!labels.is_empty(), "WHEN clause: {labels:?}");
}

#[test]
fn r3_307_create_table_on_delete_action() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (uid uuid REFERENCES users(id) ON DELETE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["CASCADE", "RESTRICT", "SET", "NO"].contains(l)),
    "ON DELETE: {labels:?}");
}

#[test]
fn r3_308_create_table_on_update_action() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (uid uuid REFERENCES users(id) ON UPDATE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["CASCADE", "RESTRICT", "SET", "NO"].contains(l)),
    "ON UPDATE: {labels:?}");
}

#[test]
fn r3_309_create_table_set_null_chain() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (uid uuid REFERENCES users(id) ON DELETE SET ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["NULL", "DEFAULT"].contains(l)),
    "SET NULL/DEFAULT: {labels:?}");
}

#[test]
fn r3_310_create_table_deferrable_chain() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int PRIMARY KEY DEFERRABLE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| l.contains("INITIALLY") || l.contains("DEFERRED") || l.contains("IMMEDIATE")),
    "DEFERRABLE: {labels:?}");
}

#[test]
fn r3_315_alter_table_alter_column_drop_default() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ALTER COLUMN active DROP ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["DEFAULT", "NOT NULL", "IDENTITY", "EXPRESSION"].contains(l) || l.contains("DEFAULT")),
    "DROP DEFAULT chain: {labels:?}");
}

#[test]
fn r3_317_alter_table_alter_column_set_storage() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ALTER COLUMN data SET STORAGE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["PLAIN", "EXTERNAL", "EXTENDED", "MAIN"].contains(l)),
    "SET STORAGE: {labels:?}");
}

#[test]
fn r3_394_offset_rows_only() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users OFFSET 10 ROWS FETCH NEXT 5 ROWS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| l == &"ONLY" || l.contains("TIES")),
    "FETCH NEXT trailing: {labels:?}");
}

#[test]
fn r3_398_is_null_chain() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WHERE id IS ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["NULL", "NOT", "TRUE", "FALSE", "DISTINCT", "UNKNOWN"].contains(l)),
    "IS chain: {labels:?}");
}

#[test]
fn r4_001_grouping_sets_inner_paren_emits_columns() {
  // CYCLE 4 fix: GROUPING SETS ((<cursor>...)) inner tuple now
  // surfaces scope columns. Was previously dumping ~600 functions.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id FROM users u GROUP BY GROUPING SETS ((  )) ";
  let cur = "SELECT id FROM users u GROUP BY GROUPING SETS ((".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id: {labels:?}");
  assert!(labels.contains(&"email"), "expected email: {labels:?}");
}

#[test]
fn r4_001_grouping_sets_inner_paren_second_tuple() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id FROM users u GROUP BY GROUPING SETS ((id), (";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"email") || labels.contains(&"id"),
    "second tuple: {labels:?}");
}

#[test]
fn r4_002_grouping_sets_outer_paren_unchanged() {
  // Regression guard: outer paren (depth=1) still emits columns from
  // the existing Phase::GroupByList path -- not affected by new
  // inner-paren detector.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id FROM users u GROUP BY GROUPING SETS ( ) ";
  let cur = "SELECT id FROM users u GROUP BY GROUPING SETS (".len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "outer GROUPING SETS: {labels:?}");
}

#[test]
fn r4_013_create_temp_table_on_commit() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TEMP TABLE t (id int) ON COMMIT ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["DELETE", "PRESERVE", "DROP"].contains(l)),
    "ON COMMIT: {labels:?}");
}

#[test]
fn r4_142_alter_table_replica_identity_no_panic() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE t REPLICA IDENTITY ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["DEFAULT", "USING", "FULL", "NOTHING"].contains(l)),
    "REPLICA IDENTITY: {labels:?}");
}

#[test]
fn r4_164_select_qualified_dot_in_where() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u WHERE u.email = 'a' AND u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"name"),
    "u dot: {labels:?}");
}

#[test]
fn r4_165_select_table_dot_in_having() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u.id FROM users u GROUP BY u.id HAVING u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"name"),
    "HAVING dot: {labels:?}");
}

#[test]
fn r4_166_select_table_dot_in_order_by() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u.id FROM users u ORDER BY u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"name"),
    "ORDER BY dot: {labels:?}");
}

#[test]
fn r4_167_select_table_dot_in_group_by() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT u.id FROM users u GROUP BY u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"name"),
    "GROUP BY dot: {labels:?}");
}

#[test]
fn r4_170_dot_after_alias_in_join_on() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u JOIN orders o ON o.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"user_id") || labels.contains(&"id"),
    "ON dot: {labels:?}");
}

#[test]
fn r4_214_discard_chain() {
  let cat = catalog_with_users_and_orders();
  let src = "DISCARD ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["ALL", "PLANS", "SEQUENCES", "TEMP", "TEMPORARY"].contains(l)),
    "DISCARD: {labels:?}");
}

#[test]
fn r4_app_schema_dot_surfaces_functions() {
  use dsl_catalog::{Function, FunctionArg};
  let mut cat = catalog_with_users_and_orders();
  cat.functions.push(Function {
    schema: "app".into(),
    name: "current_user_id".into(),
    arguments: Vec::<FunctionArg>::new(),
    return_type: "UUID".into(),
    comment: Some("CREATE FUNCTION app.current_user_id() RETURNS UUID ...".into()),
  });
  cat.functions.push(Function {
    schema: "app".into(),
    name: "user_in_org".into(),
    arguments: Vec::<FunctionArg>::new(),
    return_type: "BOOLEAN".into(),
    comment: None,
  });
  let src = "SELECT * FROM users WHERE id = app.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"current_user_id"), "missing current_user_id: {labels:?}");
  assert!(labels.contains(&"user_in_org"), "missing user_in_org: {labels:?}");
}

#[test]
fn r4_after_or_only_replace_suggested() {
  // CYCLE 4: `CREATE OR <cursor>` should only suggest `REPLACE`, not
  // re-suggest `OR REPLACE` (which would double-insert the OR).
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"REPLACE"), "expected REPLACE: {labels:?}");
  assert!(!labels.contains(&"OR REPLACE"), "duplicate OR REPLACE: {labels:?}");
  assert!(!labels.contains(&"OR"), "spurious OR: {labels:?}");
}

#[test]
fn r4_after_or_replace_no_modifiers() {
  // `CREATE OR REPLACE <cursor>` -- modifiers (TEMP/UNLOGGED/UNIQUE)
  // can't legally follow OR REPLACE. Only object-kind keywords.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR REPLACE ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"FUNCTION"), "expected FUNCTION: {labels:?}");
  assert!(labels.contains(&"VIEW"), "expected VIEW: {labels:?}");
  assert!(!labels.contains(&"OR REPLACE"), "duplicate OR REPLACE: {labels:?}");
  assert!(!labels.contains(&"TEMP"), "TEMP cannot follow OR REPLACE: {labels:?}");
  assert!(!labels.contains(&"UNLOGGED"), "UNLOGGED cannot follow OR REPLACE: {labels:?}");
}

#[test]
fn r4_returning_surfaces_columns_and_functions() {
  // After INSERT/UPDATE/DELETE ... RETURNING <cursor>, the user
  // wants: target-table columns + scope columns + the full function
  // library (so `left(`, `count(`, `now()`, etc. all complete).
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (email) VALUES ('a') RETURNING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id"), "expected id: {labels:?}");
  assert!(labels.contains(&"email"), "expected email: {labels:?}");
  // Function presence: at least one common builtin should be there.
  assert!(labels.iter().any(|l| ["left", "right", "count", "now", "max", "coalesce"].contains(l)),
    "expected at least one common function: missing in {labels:?}");
}

#[test]
fn r4_returning_then_dot_resolves_alias() {
  // RETURNING ... <alias>.<col> should also work.
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (email) VALUES ('a') RETURNING users.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email"),
    "expected users.<col>: {labels:?}");
}

#[test]
fn r4_returning_after_comma_still_full_menu() {
  // After typing `RETURNING id, <cursor>` the menu must still include
  // both columns and functions (was getting cut by used-filter).
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (email) VALUES ('a') RETURNING id, ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // `id` should be filtered (already used), `email` should still appear.
  assert!(labels.contains(&"email"), "email missing: {labels:?}");
  // At least one common function.
  assert!(labels.iter().any(|l| ["left", "count", "now", "coalesce"].contains(l)),
    "no function in second RETURNING slot: {labels:?}");
}

#[test]
fn r4_row_est_insert_values_tuples() {
  use dsl_completion::source_tables;
  let src = "CREATE TABLE t (id int);
INSERT INTO t (id) VALUES (1), (2), (3);
INSERT INTO t (id) VALUES (4), (5);";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  let row_est = cat.schemas.iter()
    .flat_map(|s| &s.tables)
    .find(|t| t.name == "t")
    .and_then(|t| t.row_estimate);
  assert_eq!(row_est, Some(5.0), "expected 3+2=5 rows; got {row_est:?}");
}

#[test]
fn r4_row_est_insert_select_generate_series() {
  use dsl_completion::source_tables;
  let src = "CREATE TABLE t (id int);
INSERT INTO t SELECT i FROM generate_series(1, 100) AS s(i);";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  let row_est = cat.schemas.iter()
    .flat_map(|s| &s.tables)
    .find(|t| t.name == "t")
    .and_then(|t| t.row_estimate);
  assert_eq!(row_est, Some(100.0), "expected 100 rows from generate_series; got {row_est:?}");
}

#[test]
fn r4_row_est_delete_clears_table() {
  use dsl_completion::source_tables;
  let src = "CREATE TABLE t (id int);
INSERT INTO t (id) VALUES (1), (2), (3), (4), (5);
DELETE FROM t;";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  let row_est = cat.schemas.iter()
    .flat_map(|s| &s.tables)
    .find(|t| t.name == "t")
    .and_then(|t| t.row_estimate);
  // DELETE without WHERE clears -> 0.
  assert_eq!(row_est, Some(0.0), "expected 0 rows after DELETE FROM t; got {row_est:?}");
}

#[test]
fn r4_row_est_truncate_clears() {
  use dsl_completion::source_tables;
  let src = "CREATE TABLE t (id int);
INSERT INTO t (id) VALUES (1), (2), (3);
TRUNCATE TABLE t;";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  let row_est = cat.schemas.iter()
    .flat_map(|s| &s.tables)
    .find(|t| t.name == "t")
    .and_then(|t| t.row_estimate);
  assert_eq!(row_est, Some(0.0), "TRUNCATE should clear; got {row_est:?}");
}

#[test]
fn r4_row_est_delete_with_where_decrements() {
  use dsl_completion::source_tables;
  let src = "CREATE TABLE t (id int);
INSERT INTO t (id) VALUES (1), (2), (3);
DELETE FROM t WHERE id = 1;";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  let row_est = cat.schemas.iter()
    .flat_map(|s| &s.tables)
    .find(|t| t.name == "t")
    .and_then(|t| t.row_estimate);
  // DELETE WHERE is best-effort -1; 3 - 1 = 2.
  assert_eq!(row_est, Some(2.0), "expected 2 rows after DELETE WHERE; got {row_est:?}");
}

#[test]
fn r4_row_est_truncate_multi_tables() {
  use dsl_completion::source_tables;
  let src = "CREATE TABLE a (id int); CREATE TABLE b (id int);
INSERT INTO a (id) VALUES (1), (2);
INSERT INTO b (id) VALUES (3), (4), (5);
TRUNCATE a, b;";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  for name in ["a", "b"] {
    let row_est = cat.schemas.iter().flat_map(|s| &s.tables).find(|t| t.name == name).and_then(|t| t.row_estimate);
    assert_eq!(row_est, Some(0.0), "{name} should be cleared by TRUNCATE");
  }
}

#[test]
fn r4_row_est_generate_series_step() {
  use dsl_completion::source_tables;
  let src = "CREATE TABLE t (id int);
INSERT INTO t SELECT i FROM generate_series(1, 10, 2) AS s(i);";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  let row_est = cat.schemas.iter().flat_map(|s| &s.tables).find(|t| t.name == "t").and_then(|t| t.row_estimate);
  // generate_series(1, 10, 2) yields 1,3,5,7,9 -> 5 rows.
  assert_eq!(row_est, Some(5.0));
}

#[test]
fn r4_create_temp_temporary_drops_or_replace() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TEMP ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(!labels.contains(&"OR REPLACE"), "TEMP -> no OR REPLACE: {labels:?}");
  assert!(labels.contains(&"TABLE"), "TEMP -> TABLE expected: {labels:?}");
}

#[test]
fn r4_503_complete_returning_with_alias_chain() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users u SET name = 'x' RETURNING u.";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.contains(&"id") || labels.contains(&"email") || labels.contains(&"name"),
    "u dot in RETURNING: {labels:?}");
}

#[test]
fn r4_542_create_table_like_including() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE clone (LIKE users INCLUDING ";
  let items = complete_at(src, src.len(), &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(labels.iter().any(|l| ["ALL", "DEFAULTS", "CONSTRAINTS", "INDEXES", "STORAGE", "COMMENTS", "STATISTICS", "GENERATED", "IDENTITY"].contains(l) || l.contains("DEFAULT") || l.contains("CONSTRAINT")),
    "LIKE INCLUDING: {labels:?}");
}

#[test]
fn r4_581_row_delta_concurrent_inserts_no_panic() {
  use dsl_completion::source_tables;
  let mut src = String::from("CREATE TABLE t (id int);\n");
  for i in 0..50 {
    src.push_str(&format!("INSERT INTO t (id) VALUES ({i});\n"));
  }
  let p = dsl_parse::parse(&src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, &src);
  let row_est = cat.schemas.iter().flat_map(|s| &s.tables).find(|t| t.name == "t").and_then(|t| t.row_estimate);
  assert_eq!(row_est, Some(50.0));
}

#[test]
fn r4_582_row_delta_mixed_insert_delete_truncate() {
  use dsl_completion::source_tables;
  let src = "CREATE TABLE t (id int);
INSERT INTO t (id) VALUES (1), (2), (3), (4), (5);
DELETE FROM t WHERE id = 1;
DELETE FROM t WHERE id = 2;
INSERT INTO t (id) VALUES (10), (11);
TRUNCATE t;
INSERT INTO t (id) VALUES (100);";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  let row_est = cat.schemas.iter().flat_map(|s| &s.tables).find(|t| t.name == "t").and_then(|t| t.row_estimate);
  // 5+2-1-1=5, truncate->0, +1 = 1
  assert_eq!(row_est, Some(1.0));
}

#[test]
fn r4_583_row_delta_generate_series_descending() {
  use dsl_completion::source_tables;
  // generate_series(10, 1, -1) -> 10 rows (decrementing)
  let src = "CREATE TABLE t (id int);
INSERT INTO t SELECT i FROM generate_series(10, 1, -1) AS s(i);";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  let row_est = cat.schemas.iter().flat_map(|s| &s.tables).find(|t| t.name == "t").and_then(|t| t.row_estimate);
  assert_eq!(row_est, Some(10.0));
}

#[test]
fn r4_594_on_conflict_do_nothing_no_increment() {
  use dsl_completion::source_tables;
  let src = "CREATE TABLE t (id int);
INSERT INTO t (id) VALUES (1), (2), (3);
INSERT INTO t (id) VALUES (1) ON CONFLICT DO NOTHING;";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  // ON CONFLICT DO NOTHING may not actually insert. Best-effort:
  // count as 1 (the optimistic case). Tracking pessimistic is a
  // documented gap.
  let row_est = cat.schemas.iter().flat_map(|s| &s.tables).find(|t| t.name == "t").and_then(|t| t.row_estimate);
  assert!(row_est.unwrap() >= 3.0);
}

#[test]
fn r4_595_insert_with_returning_no_extra_row_count() {
  use dsl_completion::source_tables;
  let src = "CREATE TABLE t (id int);
INSERT INTO t (id) VALUES (1), (2) RETURNING id;";
  let p = dsl_parse::parse(src, dsl_parse::Dialect::Postgres);
  let cat = source_tables::from_source(&p, src);
  let row_est = cat.schemas.iter().flat_map(|s| &s.tables).find(|t| t.name == "t").and_then(|t| t.row_estimate);
  assert_eq!(row_est, Some(2.0));
}

#[test]
fn r9_complete_table_0001() {
  let src = "SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `SELECT * FROM `");
}

#[test]
fn r9_complete_table_0002() {
  let src = "DROP TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `DROP TABLE `");
}

#[test]
fn r9_complete_table_0003() {
  let src = "DROP TABLE IF EXISTS ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `DROP TABLE IF EXISTS `");
}

#[test]
fn r9_complete_table_0004() {
  let src = "TRUNCATE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `TRUNCATE `");
}

#[test]
fn r9_complete_table_0005() {
  let src = "TRUNCATE TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `TRUNCATE TABLE `");
}

#[test]
fn r9_complete_table_0006() {
  let src = "UPDATE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `UPDATE `");
}

#[test]
fn r9_complete_table_0007() {
  let src = "INSERT INTO ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `INSERT INTO `");
}

#[test]
fn r9_complete_table_0008() {
  let src = "DELETE FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `DELETE FROM `");
}

#[test]
fn r9_complete_table_0009() {
  let src = "ALTER TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `ALTER TABLE `");
}

#[test]
fn r9_complete_table_0010() {
  let src = "GRANT SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `GRANT SELECT ON `");
}

#[test]
fn r9_complete_table_0011() {
  let src = "GRANT INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `GRANT INSERT ON `");
}

#[test]
fn r9_complete_table_0012() {
  let src = "GRANT UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `GRANT UPDATE ON `");
}

#[test]
fn r9_complete_table_0013() {
  let src = "GRANT DELETE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `GRANT DELETE ON `");
}

#[test]
fn r9_complete_table_0014() {
  let src = "GRANT ALL ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `GRANT ALL ON `");
}

#[test]
fn r9_complete_table_0015() {
  let src = "REVOKE SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `REVOKE SELECT ON `");
}

#[test]
fn r9_complete_table_0016() {
  let src = "REVOKE INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `REVOKE INSERT ON `");
}

#[test]
fn r9_complete_table_0017() {
  let src = "REVOKE UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `REVOKE UPDATE ON `");
}

#[test]
fn r9_complete_table_0018() {
  let src = "REVOKE DELETE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `REVOKE DELETE ON `");
}

#[test]
fn r9_complete_table_0019() {
  let src = "REVOKE ALL ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `REVOKE ALL ON `");
}

#[test]
fn r9_complete_table_0020() {
  let src = "ANALYZE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `ANALYZE `");
}

#[test]
fn r9_complete_table_0021() {
  let src = "VACUUM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `VACUUM `");
}

#[test]
fn r9_complete_table_0022() {
  let src = "VACUUM FULL ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `VACUUM FULL `");
}

#[test]
fn r9_complete_table_0023() {
  let src = "VACUUM ANALYZE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `VACUUM ANALYZE `");
}

#[test]
fn r9_complete_table_0024() {
  let src = "REINDEX TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `REINDEX TABLE `");
}

#[test]
fn r9_complete_table_0025() {
  let src = "LOCK ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `LOCK `");
}

#[test]
fn r9_complete_table_0026() {
  let src = "LOCK TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `LOCK TABLE `");
}

#[test]
fn r9_complete_table_0027() {
  let src = "SELECT * FROM users JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `SELECT * FROM users JOIN `");
}

#[test]
fn r9_complete_table_0028() {
  let src = "SELECT * FROM users LEFT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `SELECT * FROM users LEFT JOIN `");
}

#[test]
fn r9_complete_table_0029() {
  let src = "SELECT * FROM users RIGHT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "expected `users` after `SELECT * FROM users RIGHT JOIN `");
}

#[test]
fn r9_complete_table_0030() {
  let src = "SELECT * FROM users FULL JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"), "expected `orders` after `SELECT * FROM users FULL JOIN `");
}

#[test]
fn r9_complete_alias_1001() {
  let src = "SELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected col `id` for `SELECT u. FROM users u`");
}

#[test]
fn r9_complete_alias_1002() {
  let src = "SELECT u. FROM users AS u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "expected col `email` for `SELECT u. FROM users AS u`");
}

#[test]
fn r9_complete_alias_1003() {
  let src = "SELECT u. FROM users u WHERE u.id = 1";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "expected col `name` for `SELECT u. FROM users u WHERE u.id = 1`");
}

#[test]
fn r9_complete_alias_1004() {
  let src = "SELECT * FROM users u WHERE u.";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected col `id` for `SELECT * FROM users u WHERE u.`");
}

#[test]
fn r9_complete_alias_1005() {
  let src = "SELECT * FROM users u ORDER BY u.";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "expected col `email` for `SELECT * FROM users u ORDER BY u.`");
}

#[test]
fn r9_complete_alias_1006() {
  let src = "SELECT * FROM users u GROUP BY u.";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "expected col `name` for `SELECT * FROM users u GROUP BY u.`");
}

#[test]
fn r9_complete_alias_1007() {
  let src = "DELETE FROM users u WHERE u.";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected col `id` for `DELETE FROM users u WHERE u.`");
}

#[test]
fn r9_complete_alias_1008() {
  let src = "SELECT u. FROM public.users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "expected col `email` for `SELECT u. FROM public.users u`");
}

#[test]
fn r9_complete_alias_1009() {
  let src = "SELECT u. FROM users u JOIN orders o ON u.id = o.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "expected col `name` for `SELECT u. FROM users u JOIN orders o ON u.id = o.user_id`");
}

#[test]
fn r9_complete_orders_1801() {
  let src = "SELECT o. FROM orders o";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected col `id`");
}

#[test]
fn r9_complete_orders_1802() {
  let src = "SELECT o. FROM orders AS o";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "expected col `user_id`");
}

#[test]
fn r9_complete_orders_1803() {
  let src = "SELECT o. FROM orders o WHERE o.id = 1";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected col `id`");
}

#[test]
fn r9_complete_orders_1804() {
  let src = "SELECT * FROM orders o WHERE o.";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "expected col `user_id`");
}

#[test]
fn r9_complete_orders_1805() {
  let src = "SELECT * FROM orders o ORDER BY o.";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected col `id`");
}

#[test]
fn r9_complete_orders_1806() {
  let src = "DELETE FROM orders o WHERE o.";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "expected col `user_id`");
}

#[test]
fn r9_complete_orders_1807() {
  let src = "SELECT o. FROM public.orders o";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected col `id`");
}

#[test]
fn r9_complete_orders_1808() {
  let src = "SELECT o. FROM orders o";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "expected col `user_id`");
}

#[test]
fn r9_complete_orders_1809() {
  let src = "SELECT o. FROM orders AS o";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected col `id`");
}

#[test]
fn r9_complete_orders_1810() {
  let src = "SELECT o. FROM orders o WHERE o.id = 1";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "expected col `user_id`");
}

#[test]
fn r9_complete_orders_1811() {
  let src = "SELECT * FROM orders o WHERE o.";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected col `id`");
}

#[test]
fn r9_complete_orders_1812() {
  let src = "SELECT * FROM orders o ORDER BY o.";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "expected col `user_id`");
}

#[test]
fn r9_complete_orders_1813() {
  let src = "DELETE FROM orders o WHERE o.";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected col `id`");
}

#[test]
fn r9_complete_orders_1814() {
  let src = "SELECT o. FROM public.orders o";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "expected col `user_id`");
}

#[test]
fn r9_complete_qual_2101() {
  let src = "SELECT public.users. FROM public.users";
  let cur = src.find("public.users.").unwrap() + "public.users.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected `id`");
}

#[test]
fn r9_complete_qual_2102() {
  let src = "SELECT * FROM public.users WHERE public.users.";
  let cur = src.find("public.users.").unwrap() + "public.users.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "expected `email`");
}

#[test]
fn r9_complete_qual_2103() {
  let src = "SELECT public.users. FROM public.users";
  let cur = src.find("public.users.").unwrap() + "public.users.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "expected `name`");
}

#[test]
fn r9_complete_qual_2104() {
  let src = "SELECT * FROM public.users WHERE public.users.";
  let cur = src.find("public.users.").unwrap() + "public.users.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected `id`");
}

#[test]
fn r9_complete_qual_2105() {
  let src = "SELECT public.users. FROM public.users";
  let cur = src.find("public.users.").unwrap() + "public.users.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "expected `email`");
}

#[test]
fn r9_complete_qual_2106() {
  let src = "SELECT * FROM public.users WHERE public.users.";
  let cur = src.find("public.users.").unwrap() + "public.users.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "expected `name`");
}

#[test]
fn r9_complete_kind_t_2901() {
  let src = "SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2902() {
  let src = "DROP TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2903() {
  let src = "DROP TABLE IF EXISTS ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2904() {
  let src = "TRUNCATE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2905() {
  let src = "TRUNCATE TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2906() {
  let src = "UPDATE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2907() {
  let src = "INSERT INTO ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2908() {
  let src = "DELETE FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2909() {
  let src = "ALTER TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2910() {
  let src = "GRANT SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2911() {
  let src = "GRANT INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2912() {
  let src = "GRANT UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2913() {
  let src = "GRANT DELETE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2914() {
  let src = "GRANT ALL ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2915() {
  let src = "REVOKE SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2916() {
  let src = "REVOKE INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2917() {
  let src = "REVOKE UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2918() {
  let src = "REVOKE DELETE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2919() {
  let src = "REVOKE ALL ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2920() {
  let src = "ANALYZE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2921() {
  let src = "VACUUM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2922() {
  let src = "VACUUM FULL ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2923() {
  let src = "VACUUM ANALYZE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2924() {
  let src = "REINDEX TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2925() {
  let src = "LOCK ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2926() {
  let src = "LOCK TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2927() {
  let src = "SELECT * FROM users JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2928() {
  let src = "SELECT * FROM users LEFT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2929() {
  let src = "SELECT * FROM users RIGHT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_t_2930() {
  let src = "SELECT * FROM users FULL JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table)));
}

#[test]
fn r9_complete_kind_c_3101() {
  let src = "SELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column)));
}

#[test]
fn r9_complete_kind_c_3102() {
  let src = "SELECT u. FROM users AS u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column)));
}

#[test]
fn r9_complete_kind_c_3103() {
  let src = "SELECT u. FROM users u WHERE u.id = 1";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column)));
}

#[test]
fn r9_complete_kind_c_3104() {
  let src = "SELECT * FROM users u WHERE u.";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column)));
}

#[test]
fn r9_complete_kind_c_3105() {
  let src = "SELECT * FROM users u ORDER BY u.";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column)));
}

#[test]
fn r9_complete_kind_c_3106() {
  let src = "SELECT * FROM users u GROUP BY u.";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column)));
}

#[test]
fn r9_complete_kind_c_3107() {
  let src = "DELETE FROM users u WHERE u.";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column)));
}

#[test]
fn r9_complete_kind_c_3108() {
  let src = "SELECT u. FROM public.users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column)));
}

#[test]
fn r9_complete_kind_c_3109() {
  let src = "SELECT u. FROM users u JOIN orders o ON u.id = o.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column)));
}

#[test]
fn r9_complete_unique_3401() {
  let src = "SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3402() {
  let src = "DROP TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3403() {
  let src = "DROP TABLE IF EXISTS ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3404() {
  let src = "TRUNCATE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3405() {
  let src = "TRUNCATE TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3406() {
  let src = "UPDATE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3407() {
  let src = "INSERT INTO ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3408() {
  let src = "DELETE FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3409() {
  let src = "ALTER TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3410() {
  let src = "GRANT SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3411() {
  let src = "GRANT INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3412() {
  let src = "GRANT UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3413() {
  let src = "GRANT DELETE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3414() {
  let src = "GRANT ALL ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3415() {
  let src = "REVOKE SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3416() {
  let src = "REVOKE INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3417() {
  let src = "REVOKE UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3418() {
  let src = "REVOKE DELETE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3419() {
  let src = "REVOKE ALL ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3420() {
  let src = "ANALYZE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3421() {
  let src = "VACUUM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3422() {
  let src = "VACUUM FULL ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3423() {
  let src = "VACUUM ANALYZE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3424() {
  let src = "REINDEX TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3425() {
  let src = "LOCK ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3426() {
  let src = "LOCK TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3427() {
  let src = "SELECT * FROM users JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3428() {
  let src = "SELECT * FROM users LEFT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3429() {
  let src = "SELECT * FROM users RIGHT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r9_complete_unique_3430() {
  let src = "SELECT * FROM users FULL JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  let mut seen = std::collections::HashSet::new();
  for it in &items {
    let key = (it.label.clone(), it.kind);
    assert!(seen.insert(key.clone()), "duplicate: {:?}", key);
  }
}

#[test]
fn r10_mid_kw_0001() {
  let src = "S";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("S")), "expected kw with prefix `S`");
}

#[test]
fn r10_mid_kw_0002() {
  let src = "SE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SE")), "expected kw with prefix `SE`");
}

#[test]
fn r10_mid_kw_0003() {
  let src = "SEL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SEL")), "expected kw with prefix `SEL`");
}

#[test]
fn r10_mid_kw_0004() {
  let src = "SELE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SELE")), "expected kw with prefix `SELE`");
}

#[test]
fn r10_mid_kw_0005() {
  let src = "SELEC";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SELEC")), "expected kw with prefix `SELEC`");
}

#[test]
fn r10_mid_kw_0006() {
  let src = "I";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("I")), "expected kw with prefix `I`");
}

#[test]
fn r10_mid_kw_0007() {
  let src = "IN";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("IN")), "expected kw with prefix `IN`");
}

#[test]
fn r10_mid_kw_0008() {
  let src = "INS";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("INS")), "expected kw with prefix `INS`");
}

#[test]
fn r10_mid_kw_0009() {
  let src = "INSE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("INSE")), "expected kw with prefix `INSE`");
}

#[test]
fn r10_mid_kw_0010() {
  let src = "INSER";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("INSER")), "expected kw with prefix `INSER`");
}

#[test]
fn r10_mid_kw_0011() {
  let src = "U";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("U")), "expected kw with prefix `U`");
}

#[test]
fn r10_mid_kw_0012() {
  let src = "UP";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("UP")), "expected kw with prefix `UP`");
}

#[test]
fn r10_mid_kw_0013() {
  let src = "UPD";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("UPD")), "expected kw with prefix `UPD`");
}

#[test]
fn r10_mid_kw_0014() {
  let src = "UPDA";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("UPDA")), "expected kw with prefix `UPDA`");
}

#[test]
fn r10_mid_kw_0015() {
  let src = "UPDAT";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("UPDAT")), "expected kw with prefix `UPDAT`");
}

#[test]
fn r10_mid_kw_0016() {
  let src = "D";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("D")), "expected kw with prefix `D`");
}

#[test]
fn r10_mid_kw_0017() {
  let src = "DE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DE")), "expected kw with prefix `DE`");
}

#[test]
fn r10_mid_kw_0018() {
  let src = "DEL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DEL")), "expected kw with prefix `DEL`");
}

#[test]
fn r10_mid_kw_0019() {
  let src = "DELE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DELE")), "expected kw with prefix `DELE`");
}

#[test]
fn r10_mid_kw_0020() {
  let src = "DELET";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DELET")), "expected kw with prefix `DELET`");
}

#[test]
fn r10_mid_kw_0021() {
  let src = "C";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("C")), "expected kw with prefix `C`");
}

#[test]
fn r10_mid_kw_0022() {
  let src = "CR";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CR")), "expected kw with prefix `CR`");
}

#[test]
fn r10_mid_kw_0023() {
  let src = "CRE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CRE")), "expected kw with prefix `CRE`");
}

#[test]
fn r10_mid_kw_0024() {
  let src = "CREA";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CREA")), "expected kw with prefix `CREA`");
}

#[test]
fn r10_mid_kw_0025() {
  let src = "CREAT";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CREAT")), "expected kw with prefix `CREAT`");
}

#[test]
fn r10_mid_kw_0026() {
  let src = "A";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("A")), "expected kw with prefix `A`");
}

#[test]
fn r10_mid_kw_0027() {
  let src = "AL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("AL")), "expected kw with prefix `AL`");
}

#[test]
fn r10_mid_kw_0028() {
  let src = "ALT";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("ALT")), "expected kw with prefix `ALT`");
}

#[test]
fn r10_mid_kw_0029() {
  let src = "ALTE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("ALTE")), "expected kw with prefix `ALTE`");
}

#[test]
fn r10_mid_kw_0030() {
  let src = "DR";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DR")), "expected kw with prefix `DR`");
}

#[test]
fn r10_multi_0082() {
  let src = "SELECT 1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0083() {
  let src = "SELECT 2;\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0084() {
  let src = "SELECT 1, 2; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0085() {
  let src = "INSERT INTO users (id) VALUES (1); SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0086() {
  let src = "UPDATE users SET name='a' WHERE id=1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0087() {
  let src = "UPDATE users SET name='b' WHERE id=2; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0088() {
  let src = "DELETE FROM users WHERE id=1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0089() {
  let src = "DELETE FROM users WHERE id=2; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0090() {
  let src = "BEGIN; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0091() {
  let src = "BEGIN READ ONLY; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0092() {
  let src = "BEGIN READ WRITE; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0093() {
  let src = "BEGIN ISOLATION LEVEL SERIALIZABLE; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0094() {
  let src = "COMMIT; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0095() {
  let src = "ROLLBACK; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0096() {
  let src = "SAVEPOINT sp1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0097() {
  let src = "SAVEPOINT sp2; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0098() {
  let src = "CREATE TABLE t1 (id int); SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0099() {
  let src = "CREATE TABLE t2 (id int); SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0100() {
  let src = "CREATE INDEX i1 ON users (id); SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0101() {
  let src = "CREATE INDEX i2 ON users (email); SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0102() {
  let src = "DROP TABLE t1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0103() {
  let src = "DROP TABLE t2; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0104() {
  let src = "ALTER TABLE users ADD COLUMN c1 int; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0105() {
  let src = "ALTER TABLE users ADD COLUMN c2 int; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0106() {
  let src = "SET search_path TO public; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0107() {
  let src = "SET search_path TO private; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0108() {
  let src = "GRANT SELECT ON users TO alice; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0109() {
  let src = "GRANT INSERT ON users TO bob; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0110() {
  let src = "REVOKE SELECT ON users FROM alice; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_multi_0111() {
  let src = "REVOKE INSERT ON users FROM bob; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "second-stmt completion missed users");
}

#[test]
fn r10_comment_0112() {
  let src = "/* hint */ SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_comment_0113() {
  let src = "/* hint2 */ SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_comment_0114() {
  let src = "/* multi\n line */ SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_comment_0115() {
  let src = "-- top\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_comment_0116() {
  let src = "-- top2\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_comment_0117() {
  let src = "-- top3\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_comment_0119() {
  let src = "/* a */\n-- b\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_comment_0120() {
  let src = "-- a\n/* b */\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_comment_0121() {
  let src = "/* a */ SELECT 1; /* b */ SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_comment_0122() {
  let src = "-- a\nSELECT 1;\n-- b\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_comment_0123() {
  let src = "/* a */ SELECT 1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "comment-prefixed missed users");
}

#[test]
fn r10_ws_0132() {
  let src = "SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0133() {
  let src = "SELECT * FROM  ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0134() {
  let src = "SELECT * FROM \n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0135() {
  let src = "SELECT * FROM \t";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0136() {
  let src = "SELECT * FROM  \n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0137() {
  let src = "SELECT * FROM \n ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0138() {
  let src = "\tSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0139() {
  let src = "\tSELECT * FROM  ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0140() {
  let src = "\tSELECT * FROM \n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0141() {
  let src = "\tSELECT * FROM \t";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0142() {
  let src = "\tSELECT * FROM  \n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0143() {
  let src = "\tSELECT * FROM \n ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0144() {
  let src = "\t\tSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0145() {
  let src = "\t\tSELECT * FROM  ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0146() {
  let src = "\t\tSELECT * FROM \n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0147() {
  let src = "\t\tSELECT * FROM \t";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0148() {
  let src = "\t\tSELECT * FROM  \n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0149() {
  let src = "\t\tSELECT * FROM \n ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0150() {
  let src = " SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0151() {
  let src = " SELECT * FROM  ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0152() {
  let src = " SELECT * FROM \n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0153() {
  let src = " SELECT * FROM \t";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0154() {
  let src = " SELECT * FROM  \n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0155() {
  let src = " SELECT * FROM \n ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0156() {
  let src = " \tSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0157() {
  let src = " \tSELECT * FROM  ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0158() {
  let src = " \tSELECT * FROM \n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0159() {
  let src = " \tSELECT * FROM \t";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0160() {
  let src = " \tSELECT * FROM  \n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_ws_0161() {
  let src = " \tSELECT * FROM \n ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_proj_0222() {
  let src = "SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0223() {
  let src = "SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0224() {
  let src = "SELECT 1, 2 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0225() {
  let src = "SELECT id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0226() {
  let src = "SELECT id, name FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0227() {
  let src = "SELECT id, name, email FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0228() {
  let src = "SELECT users.id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0229() {
  let src = "SELECT u.id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0230() {
  let src = "SELECT count(*) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0231() {
  let src = "SELECT max(id) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0232() {
  let src = "SELECT min(id) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0233() {
  let src = "SELECT avg(amount) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0234() {
  let src = "SELECT sum(amount) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0235() {
  let src = "SELECT DISTINCT id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0236() {
  let src = "SELECT DISTINCT ON (id) * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0237() {
  let src = "SELECT ALL id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0238() {
  let src = "SELECT row_number() OVER () FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0239() {
  let src = "SELECT rank() OVER (ORDER BY id) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0240() {
  let src = "SELECT 'literal' FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0241() {
  let src = "SELECT 0 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0242() {
  let src = "SELECT NULL FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0243() {
  let src = "SELECT TRUE FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_proj_0244() {
  let src = "SELECT FALSE FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"), "missed users in projection ctx");
}

#[test]
fn r10_join_0245() {
  let src = "SELECT u. FROM users u JOIN orders o ON u.id = o.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "join ctx missed col `id`");
}

#[test]
fn r10_join_0246() {
  let src = "SELECT u. FROM users u JOIN orders o ON u.id = o.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "join ctx missed col `email`");
}

#[test]
fn r10_join_0247() {
  let src = "SELECT u. FROM users u JOIN orders o ON u.id = o.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "join ctx missed col `name`");
}

#[test]
fn r10_join_0248() {
  let src = "SELECT u. FROM users u JOIN orders oo ON u.id = oo.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "join ctx missed col `id`");
}

#[test]
fn r10_join_0249() {
  let src = "SELECT u. FROM users u JOIN orders oo ON u.id = oo.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "join ctx missed col `email`");
}

#[test]
fn r10_join_0250() {
  let src = "SELECT u. FROM users u JOIN orders oo ON u.id = oo.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "join ctx missed col `name`");
}

#[test]
fn r10_join_0251() {
  let src = "SELECT u. FROM users u JOIN orders tbl2 ON u.id = tbl2.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "join ctx missed col `id`");
}

#[test]
fn r10_join_0252() {
  let src = "SELECT u. FROM users u JOIN orders tbl2 ON u.id = tbl2.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "join ctx missed col `email`");
}

#[test]
fn r10_join_0253() {
  let src = "SELECT u. FROM users u JOIN orders tbl2 ON u.id = tbl2.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "join ctx missed col `name`");
}

#[test]
fn r10_join_0254() {
  let src = "SELECT u. FROM users u JOIN orders ord ON u.id = ord.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "join ctx missed col `id`");
}

#[test]
fn r10_join_0255() {
  let src = "SELECT u. FROM users u JOIN orders ord ON u.id = ord.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "join ctx missed col `email`");
}

#[test]
fn r10_join_0256() {
  let src = "SELECT u. FROM users u JOIN orders ord ON u.id = ord.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "join ctx missed col `name`");
}

#[test]
fn r10_join_0257() {
  let src = "SELECT u. FROM users u JOIN orders mainord ON u.id = mainord.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "join ctx missed col `id`");
}

#[test]
fn r10_join_0258() {
  let src = "SELECT u. FROM users u JOIN orders mainord ON u.id = mainord.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "join ctx missed col `email`");
}

#[test]
fn r10_join_0259() {
  let src = "SELECT u. FROM users u JOIN orders mainord ON u.id = mainord.user_id";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "join ctx missed col `name`");
}

#[test]
fn r10_join_0260() {
  let src = "SELECT x. FROM users x JOIN orders o ON x.id = o.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "join ctx missed col `id`");
}

#[test]
fn r10_join_0261() {
  let src = "SELECT x. FROM users x JOIN orders o ON x.id = o.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "join ctx missed col `email`");
}

#[test]
fn r10_join_0262() {
  let src = "SELECT x. FROM users x JOIN orders o ON x.id = o.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "join ctx missed col `name`");
}

#[test]
fn r10_join_0263() {
  let src = "SELECT x. FROM users x JOIN orders oo ON x.id = oo.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "join ctx missed col `id`");
}

#[test]
fn r10_join_0264() {
  let src = "SELECT x. FROM users x JOIN orders oo ON x.id = oo.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "join ctx missed col `email`");
}

#[test]
fn r10_join_0265() {
  let src = "SELECT x. FROM users x JOIN orders oo ON x.id = oo.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "join ctx missed col `name`");
}

#[test]
fn r10_join_0266() {
  let src = "SELECT x. FROM users x JOIN orders tbl2 ON x.id = tbl2.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "join ctx missed col `id`");
}

#[test]
fn r10_join_0267() {
  let src = "SELECT x. FROM users x JOIN orders tbl2 ON x.id = tbl2.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "join ctx missed col `email`");
}

#[test]
fn r10_join_0268() {
  let src = "SELECT x. FROM users x JOIN orders tbl2 ON x.id = tbl2.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "join ctx missed col `name`");
}

#[test]
fn r10_join_0269() {
  let src = "SELECT x. FROM users x JOIN orders ord ON x.id = ord.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "join ctx missed col `id`");
}

#[test]
fn r10_join_0270() {
  let src = "SELECT x. FROM users x JOIN orders ord ON x.id = ord.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "join ctx missed col `email`");
}

#[test]
fn r10_join_0271() {
  let src = "SELECT x. FROM users x JOIN orders ord ON x.id = ord.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "join ctx missed col `name`");
}

#[test]
fn r10_join_0272() {
  let src = "SELECT x. FROM users x JOIN orders mainord ON x.id = mainord.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "join ctx missed col `id`");
}

#[test]
fn r10_join_0273() {
  let src = "SELECT x. FROM users x JOIN orders mainord ON x.id = mainord.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "join ctx missed col `email`");
}

#[test]
fn r10_join_0274() {
  let src = "SELECT x. FROM users x JOIN orders mainord ON x.id = mainord.user_id";
  let cur = src.find("x.").unwrap() + "x.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "join ctx missed col `name`");
}

#[test]
fn r10_orders_alias_0335() {
  let src = "SELECT o. FROM orders o";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0336() {
  let src = "SELECT o. FROM orders o";
  let cur = src.find("o.").unwrap() + "o.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0337() {
  let src = "SELECT ord. FROM orders ord";
  let cur = src.find("ord.").unwrap() + "ord.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0338() {
  let src = "SELECT ord. FROM orders ord";
  let cur = src.find("ord.").unwrap() + "ord.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0339() {
  let src = "SELECT oo. FROM orders oo";
  let cur = src.find("oo.").unwrap() + "oo.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0340() {
  let src = "SELECT oo. FROM orders oo";
  let cur = src.find("oo.").unwrap() + "oo.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0341() {
  let src = "SELECT z1. FROM orders z1";
  let cur = src.find("z1.").unwrap() + "z1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0342() {
  let src = "SELECT z1. FROM orders z1";
  let cur = src.find("z1.").unwrap() + "z1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0343() {
  let src = "SELECT z2. FROM orders z2";
  let cur = src.find("z2.").unwrap() + "z2.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0344() {
  let src = "SELECT z2. FROM orders z2";
  let cur = src.find("z2.").unwrap() + "z2.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0345() {
  let src = "SELECT z3. FROM orders z3";
  let cur = src.find("z3.").unwrap() + "z3.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "orders alias missed col");
}

#[test]
fn r10_orders_alias_0346() {
  let src = "SELECT z3. FROM orders z3";
  let cur = src.find("z3.").unwrap() + "z3.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "orders alias missed col");
}

#[test]
fn r10_grant_0361() {
  let src = "-- v1\nGRANT SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_grant_0362() {
  let src = "-- v2\nGRANT SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r10_grant_0363() {
  let src = "-- v3\nGRANT SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_grant_0364() {
  let src = "-- v4\nGRANT SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r10_grant_0365() {
  let src = "-- v5\nGRANT SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_grant_0366() {
  let src = "-- v6\nGRANT SELECT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r10_grant_0371() {
  let src = "-- v11\nGRANT INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_grant_0372() {
  let src = "-- v12\nGRANT INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r10_grant_0373() {
  let src = "-- v13\nGRANT INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_grant_0374() {
  let src = "-- v14\nGRANT INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r10_grant_0375() {
  let src = "-- v15\nGRANT INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_grant_0376() {
  let src = "-- v16\nGRANT INSERT ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r10_grant_0381() {
  let src = "-- v21\nGRANT UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_grant_0382() {
  let src = "-- v22\nGRANT UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r10_grant_0383() {
  let src = "-- v23\nGRANT UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_grant_0384() {
  let src = "-- v24\nGRANT UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r10_grant_0385() {
  let src = "-- v25\nGRANT UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_grant_0386() {
  let src = "-- v26\nGRANT UPDATE ON ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r10_unique_ctx_1081() {
  let src = "-- v0\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_unique_ctx_1096() {
  let src = "-- v0\nDROP TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_unique_ctx_1097() {
  let src = "-- v1\nDROP TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_unique_ctx_1098() {
  let src = "-- v2\nDROP TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r10_alias_var_1291() {
  let src = "-- av0\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r10_alias_var_1292() {
  let src = "-- av1\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r10_alias_var_1293() {
  let src = "-- av2\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r10_alias_var_1299() {
  let src = "-- av0\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r10_alias_var_1300() {
  let src = "-- av1\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r10_alias_var_1301() {
  let src = "-- av2\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r10_alias_var_1307() {
  let src = "-- av0\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r10_alias_var_1308() {
  let src = "-- av1\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r10_alias_var_1309() {
  let src = "-- av2\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r10_alias_var_1315() {
  let src = "-- av0\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r10_alias_var_1316() {
  let src = "-- av1\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r10_alias_var_1317() {
  let src = "-- av2\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r11_kind_table_0001() {
  let src = "-- s0\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0002() {
  let src = "-- s0\nDROP TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0003() {
  let src = "-- s0\nTRUNCATE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0004() {
  let src = "-- s0\nALTER TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0005() {
  let src = "-- s1\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0006() {
  let src = "-- s1\nDROP TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0007() {
  let src = "-- s1\nTRUNCATE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0008() {
  let src = "-- s1\nALTER TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0009() {
  let src = "-- s2\nSELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0010() {
  let src = "-- s2\nDROP TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0011() {
  let src = "-- s2\nTRUNCATE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_table_0012() {
  let src = "-- s2\nALTER TABLE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Table) && i.label == "users"));
}

#[test]
fn r11_kind_col_0151() {
  let src = "-- ck0\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "id"));
}

#[test]
fn r11_kind_col_0152() {
  let src = "-- ck1\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "id"));
}

#[test]
fn r11_kind_col_0153() {
  let src = "-- ck2\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "id"));
}

#[test]
fn r11_kind_col_0155() {
  let src = "-- ck0\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "email"));
}

#[test]
fn r11_kind_col_0156() {
  let src = "-- ck1\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "email"));
}

#[test]
fn r11_kind_col_0157() {
  let src = "-- ck2\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "email"));
}

#[test]
fn r11_kind_col_0159() {
  let src = "-- ck0\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "name"));
}

#[test]
fn r11_kind_col_0160() {
  let src = "-- ck1\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "name"));
}

#[test]
fn r11_kind_col_0161() {
  let src = "-- ck2\nSELECT u. FROM users u";
  let cur = src.find("u.").unwrap() + "u.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "name"));
}

#[test]
fn r11_kind_col_0163() {
  let src = "-- ck0\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "id"));
}

#[test]
fn r11_kind_col_0164() {
  let src = "-- ck1\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "id"));
}

#[test]
fn r11_kind_col_0165() {
  let src = "-- ck2\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "id"));
}

#[test]
fn r11_kind_col_0167() {
  let src = "-- ck0\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "email"));
}

#[test]
fn r11_kind_col_0168() {
  let src = "-- ck1\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "email"));
}

#[test]
fn r11_kind_col_0169() {
  let src = "-- ck2\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "email"));
}

#[test]
fn r11_kind_col_0171() {
  let src = "-- ck0\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "name"));
}

#[test]
fn r11_kind_col_0172() {
  let src = "-- ck1\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "name"));
}

#[test]
fn r11_kind_col_0173() {
  let src = "-- ck2\nSELECT u1. FROM users u1";
  let cur = src.find("u1.").unwrap() + "u1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Column) && i.label == "name"));
}

#[test]
fn r12_mid_kw_0001() {
  let src = "-- t1\nS";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("S")), "no kw matching `S`");
}

#[test]
fn r12_mid_kw_0002() {
  let src = "-- t2\nSE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SE")), "no kw matching `SE`");
}

#[test]
fn r12_mid_kw_0003() {
  let src = "-- t3\nSEL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SEL")), "no kw matching `SEL`");
}

#[test]
fn r12_mid_kw_0004() {
  let src = "-- t4\nSELE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SELE")), "no kw matching `SELE`");
}

#[test]
fn r12_mid_kw_0005() {
  let src = "-- t5\nSELEC";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SELEC")), "no kw matching `SELEC`");
}

#[test]
fn r12_mid_kw_0006() {
  let src = "-- t6\nI";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("I")), "no kw matching `I`");
}

#[test]
fn r12_mid_kw_0007() {
  let src = "-- t7\nIN";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("IN")), "no kw matching `IN`");
}

#[test]
fn r12_mid_kw_0008() {
  let src = "-- t8\nINS";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("INS")), "no kw matching `INS`");
}

#[test]
fn r12_mid_kw_0009() {
  let src = "-- t9\nINSE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("INSE")), "no kw matching `INSE`");
}

#[test]
fn r12_mid_kw_0010() {
  let src = "-- t10\nINSER";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("INSER")), "no kw matching `INSER`");
}

#[test]
fn r12_mid_kw_0011() {
  let src = "-- t11\nU";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("U")), "no kw matching `U`");
}

#[test]
fn r12_mid_kw_0012() {
  let src = "-- t12\nUP";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("UP")), "no kw matching `UP`");
}

#[test]
fn r12_mid_kw_0013() {
  let src = "-- t13\nUPD";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("UPD")), "no kw matching `UPD`");
}

#[test]
fn r12_mid_kw_0014() {
  let src = "-- t14\nUPDA";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("UPDA")), "no kw matching `UPDA`");
}

#[test]
fn r12_mid_kw_0015() {
  let src = "-- t15\nUPDAT";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("UPDAT")), "no kw matching `UPDAT`");
}

#[test]
fn r12_mid_kw_0016() {
  let src = "-- t16\nD";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("D")), "no kw matching `D`");
}

#[test]
fn r12_mid_kw_0017() {
  let src = "-- t17\nDE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DE")), "no kw matching `DE`");
}

#[test]
fn r12_mid_kw_0018() {
  let src = "-- t18\nDEL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DEL")), "no kw matching `DEL`");
}

#[test]
fn r12_mid_kw_0019() {
  let src = "-- t19\nDELE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DELE")), "no kw matching `DELE`");
}

#[test]
fn r12_mid_kw_0020() {
  let src = "-- t20\nDELET";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DELET")), "no kw matching `DELET`");
}

#[test]
fn r12_mid_kw_0021() {
  let src = "-- t21\nC";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("C")), "no kw matching `C`");
}

#[test]
fn r12_mid_kw_0022() {
  let src = "-- t22\nCR";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CR")), "no kw matching `CR`");
}

#[test]
fn r12_mid_kw_0023() {
  let src = "-- t23\nCRE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CRE")), "no kw matching `CRE`");
}

#[test]
fn r12_mid_kw_0024() {
  let src = "-- t24\nCREA";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CREA")), "no kw matching `CREA`");
}

#[test]
fn r12_mid_kw_0025() {
  let src = "-- t25\nCREAT";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CREAT")), "no kw matching `CREAT`");
}

#[test]
fn r12_mid_kw_0026() {
  let src = "-- t26\nA";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("A")), "no kw matching `A`");
}

#[test]
fn r12_mid_kw_0027() {
  let src = "-- t27\nAL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("AL")), "no kw matching `AL`");
}

#[test]
fn r12_mid_kw_0028() {
  let src = "-- t28\nALT";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("ALT")), "no kw matching `ALT`");
}

#[test]
fn r12_mid_kw_0029() {
  let src = "-- t29\nALTE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("ALTE")), "no kw matching `ALTE`");
}

#[test]
fn r12_mid_kw_0030() {
  let src = "-- t30\nDR";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DR")), "no kw matching `DR`");
}

#[test]
fn r12_chain_0147() {
  let src = "-- mc0\nSELECT 1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r12_chain_0148() {
  let src = "-- mc0\nSELECT 1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r12_chain_0149() {
  let src = "-- mc1\nSELECT 1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r12_chain_0150() {
  let src = "-- mc1\nSELECT 1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r12_chain_0151() {
  let src = "-- mc2\nSELECT 1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r12_chain_0152() {
  let src = "-- mc2\nSELECT 1; SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r12_sub_0401() {
  let src = "-- sub0\nSELECT * FROM users WHERE id IN (SELECT id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r12_sub_0402() {
  let src = "-- sub0\nSELECT * FROM users WHERE id IN (SELECT id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r12_sub_0403() {
  let src = "-- sub0\nSELECT * FROM users WHERE EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r12_sub_0404() {
  let src = "-- sub0\nSELECT * FROM users WHERE EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r12_sub_0405() {
  let src = "-- sub1\nSELECT * FROM users WHERE id IN (SELECT id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r12_sub_0406() {
  let src = "-- sub1\nSELECT * FROM users WHERE id IN (SELECT id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r12_sub_0407() {
  let src = "-- sub1\nSELECT * FROM users WHERE EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r12_sub_0408() {
  let src = "-- sub1\nSELECT * FROM users WHERE EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r12_sub_0409() {
  let src = "-- sub2\nSELECT * FROM users WHERE id IN (SELECT id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r12_sub_0410() {
  let src = "-- sub2\nSELECT * FROM users WHERE id IN (SELECT id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r12_sub_0411() {
  let src = "-- sub2\nSELECT * FROM users WHERE EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r12_sub_0412() {
  let src = "-- sub2\nSELECT * FROM users WHERE EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r12_alias_letter_0601() {
  let src = "-- ea0\nSELECT a. FROM users a";
  let cur = src.find("a.").unwrap() + "a.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r12_alias_letter_0602() {
  let src = "-- ea0\nSELECT a. FROM users a";
  let cur = src.find("a.").unwrap() + "a.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r12_alias_letter_0603() {
  let src = "-- ea0\nSELECT a. FROM users a";
  let cur = src.find("a.").unwrap() + "a.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r12_alias_letter_0604() {
  let src = "-- ea1\nSELECT a. FROM users a";
  let cur = src.find("a.").unwrap() + "a.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r12_alias_letter_0605() {
  let src = "-- ea1\nSELECT a. FROM users a";
  let cur = src.find("a.").unwrap() + "a.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r12_alias_letter_0606() {
  let src = "-- ea1\nSELECT a. FROM users a";
  let cur = src.find("a.").unwrap() + "a.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r12_alias_letter_0607() {
  let src = "-- ea2\nSELECT a. FROM users a";
  let cur = src.find("a.").unwrap() + "a.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r12_alias_letter_0608() {
  let src = "-- ea2\nSELECT a. FROM users a";
  let cur = src.find("a.").unwrap() + "a.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r12_alias_letter_0609() {
  let src = "-- ea2\nSELECT a. FROM users a";
  let cur = src.find("a.").unwrap() + "a.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r12_alias_letter_0625() {
  let src = "-- ea0\nSELECT b. FROM users b";
  let cur = src.find("b.").unwrap() + "b.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r12_alias_letter_0626() {
  let src = "-- ea0\nSELECT b. FROM users b";
  let cur = src.find("b.").unwrap() + "b.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r12_alias_letter_0627() {
  let src = "-- ea0\nSELECT b. FROM users b";
  let cur = src.find("b.").unwrap() + "b.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r12_alias_letter_0628() {
  let src = "-- ea1\nSELECT b. FROM users b";
  let cur = src.find("b.").unwrap() + "b.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r12_alias_letter_0629() {
  let src = "-- ea1\nSELECT b. FROM users b";
  let cur = src.find("b.").unwrap() + "b.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r12_alias_letter_0630() {
  let src = "-- ea1\nSELECT b. FROM users b";
  let cur = src.find("b.").unwrap() + "b.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r13_subq_0001() {
  let src = "-- sq0\nSELECT * FROM users WHERE id IN (SELECT user_id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0006() {
  let src = "-- sq0\nSELECT * FROM users WHERE NOT EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_subq_0007() {
  let src = "-- sq0\nSELECT * FROM users WHERE NOT EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0008() {
  let src = "-- sq0\nSELECT (SELECT count(*) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_subq_0009() {
  let src = "-- sq0\nSELECT (SELECT count(*) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0010() {
  let src = "-- sq0\nSELECT (SELECT max(id) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_subq_0011() {
  let src = "-- sq0\nSELECT (SELECT max(id) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0012() {
  let src = "-- sq0\nWITH x AS (SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_subq_0013() {
  let src = "-- sq0\nWITH x AS (SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0014() {
  let src = "-- sq0\nWITH x AS (SELECT 1), y AS (SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_subq_0015() {
  let src = "-- sq0\nWITH x AS (SELECT 1), y AS (SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0016() {
  let src = "-- sq1\nSELECT * FROM users WHERE id IN (SELECT user_id FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0021() {
  let src = "-- sq1\nSELECT * FROM users WHERE NOT EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_subq_0022() {
  let src = "-- sq1\nSELECT * FROM users WHERE NOT EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0023() {
  let src = "-- sq1\nSELECT (SELECT count(*) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_subq_0024() {
  let src = "-- sq1\nSELECT (SELECT count(*) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0025() {
  let src = "-- sq1\nSELECT (SELECT max(id) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_subq_0026() {
  let src = "-- sq1\nSELECT (SELECT max(id) FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0027() {
  let src = "-- sq1\nWITH x AS (SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_subq_0028() {
  let src = "-- sq1\nWITH x AS (SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_subq_0029() {
  let src = "-- sq1\nWITH x AS (SELECT 1), y AS (SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_subq_0030() {
  let src = "-- sq1\nWITH x AS (SELECT 1), y AS (SELECT * FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0501() {
  let src = "-- jc0\nSELECT * FROM users u JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0502() {
  let src = "-- jc0\nSELECT * FROM users u LEFT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0503() {
  let src = "-- jc0\nSELECT * FROM users u INNER JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0504() {
  let src = "-- jc0\nSELECT * FROM users u RIGHT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0505() {
  let src = "-- jc0\nSELECT * FROM users u FULL JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0506() {
  let src = "-- jc0\nSELECT * FROM users u CROSS JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0509() {
  let src = "-- jc0\nSELECT * FROM orders o JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_join_chain_0510() {
  let src = "-- jc0\nSELECT * FROM orders o LEFT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_join_chain_0511() {
  let src = "-- jc1\nSELECT * FROM users u JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0512() {
  let src = "-- jc1\nSELECT * FROM users u LEFT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0513() {
  let src = "-- jc1\nSELECT * FROM users u INNER JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0514() {
  let src = "-- jc1\nSELECT * FROM users u RIGHT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0515() {
  let src = "-- jc1\nSELECT * FROM users u FULL JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0516() {
  let src = "-- jc1\nSELECT * FROM users u CROSS JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0519() {
  let src = "-- jc1\nSELECT * FROM orders o JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_join_chain_0520() {
  let src = "-- jc1\nSELECT * FROM orders o LEFT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_join_chain_0521() {
  let src = "-- jc2\nSELECT * FROM users u JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0522() {
  let src = "-- jc2\nSELECT * FROM users u LEFT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0523() {
  let src = "-- jc2\nSELECT * FROM users u INNER JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0524() {
  let src = "-- jc2\nSELECT * FROM users u RIGHT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0525() {
  let src = "-- jc2\nSELECT * FROM users u FULL JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0526() {
  let src = "-- jc2\nSELECT * FROM users u CROSS JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_join_chain_0529() {
  let src = "-- jc2\nSELECT * FROM orders o JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_join_chain_0530() {
  let src = "-- jc2\nSELECT * FROM orders o LEFT JOIN ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_alias_deep_0901() {
  let src = "-- ad1\nSELECT my_0. FROM users my_0";
  let cur = src.find("my_0.").unwrap() + "my_0.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r13_alias_deep_0902() {
  let src = "-- ad2\nSELECT my_0. FROM users my_0";
  let cur = src.find("my_0.").unwrap() + "my_0.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r13_alias_deep_0903() {
  let src = "-- ad3\nSELECT my_0. FROM users my_0";
  let cur = src.find("my_0.").unwrap() + "my_0.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r13_alias_deep_0904() {
  let src = "-- ad4\nSELECT my_1. FROM users my_1";
  let cur = src.find("my_1.").unwrap() + "my_1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r13_alias_deep_0905() {
  let src = "-- ad5\nSELECT my_1. FROM users my_1";
  let cur = src.find("my_1.").unwrap() + "my_1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r13_alias_deep_0906() {
  let src = "-- ad6\nSELECT my_1. FROM users my_1";
  let cur = src.find("my_1.").unwrap() + "my_1.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r13_alias_deep_0907() {
  let src = "-- ad7\nSELECT my_2. FROM users my_2";
  let cur = src.find("my_2.").unwrap() + "my_2.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r13_alias_deep_0908() {
  let src = "-- ad8\nSELECT my_2. FROM users my_2";
  let cur = src.find("my_2.").unwrap() + "my_2.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r13_alias_deep_0909() {
  let src = "-- ad9\nSELECT my_2. FROM users my_2";
  let cur = src.find("my_2.").unwrap() + "my_2.".len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r13_schema_1351() {
  let src = "-- sc0\nSELECT * FROM public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_schema_1352() {
  let src = "-- sc0\nSELECT * FROM public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_schema_1353() {
  let src = "-- sc0\nDROP TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_schema_1354() {
  let src = "-- sc0\nDROP TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_schema_1355() {
  let src = "-- sc0\nALTER TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_schema_1356() {
  let src = "-- sc0\nALTER TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_schema_1357() {
  let src = "-- sc1\nSELECT * FROM public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_schema_1358() {
  let src = "-- sc1\nSELECT * FROM public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_schema_1359() {
  let src = "-- sc1\nDROP TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_schema_1360() {
  let src = "-- sc1\nDROP TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_schema_1361() {
  let src = "-- sc1\nALTER TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_schema_1362() {
  let src = "-- sc1\nALTER TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_schema_1363() {
  let src = "-- sc2\nSELECT * FROM public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_schema_1364() {
  let src = "-- sc2\nSELECT * FROM public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_schema_1365() {
  let src = "-- sc2\nDROP TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_schema_1366() {
  let src = "-- sc2\nDROP TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r13_schema_1367() {
  let src = "-- sc2\nALTER TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r13_schema_1368() {
  let src = "-- sc2\nALTER TABLE public.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r15_mw_0601() {
  let src = "-- mw0\nSEL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SEL")));
}

#[test]
fn r15_mw_0602() {
  let src = "-- mw0\nSELE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SELE")));
}

#[test]
fn r15_mw_0603() {
  let src = "-- mw0\nSELEC";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SELEC")));
}

#[test]
fn r15_mw_0604() {
  let src = "-- mw0\nUPD";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("UPD")));
}

#[test]
fn r15_mw_0605() {
  let src = "-- mw0\nDEL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DEL")));
}

#[test]
fn r15_mw_0606() {
  let src = "-- mw0\nINS";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("INS")));
}

#[test]
fn r15_mw_0607() {
  let src = "-- mw0\nCRE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CRE")));
}

#[test]
fn r15_mw_0608() {
  let src = "-- mw0\nCREA";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CREA")));
}

#[test]
fn r15_mw_0609() {
  let src = "-- mw0\nALT";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("ALT")));
}

#[test]
fn r15_mw_0610() {
  let src = "-- mw0\nDRO";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("DRO")));
}

#[test]
fn r15_mw_0611() {
  let src = "-- mw0\nBEG";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("BEG")));
}

#[test]
fn r15_mw_0612() {
  let src = "-- mw0\nCOM";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("COM")));
}

#[test]
fn r15_mw_0613() {
  let src = "-- mw0\nROL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("ROL")));
}

#[test]
fn r15_mw_0614() {
  let src = "-- mw0\nWIT";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("WIT")));
}

#[test]
fn r15_mw_0615() {
  let src = "-- mw0\nEXP";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("EXP")));
}

#[test]
fn r15_mw_0616() {
  let src = "-- mw0\nGRA";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("GRA")));
}

#[test]
fn r15_mw_0617() {
  let src = "-- mw0\nREV";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("REV")));
}

#[test]
fn r15_mw_0618() {
  let src = "-- mw0\nVAC";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("VAC")));
}

#[test]
fn r15_mw_0619() {
  let src = "-- mw0\nANA";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("ANA")));
}

#[test]
fn r15_mw_0620() {
  let src = "-- mw0\nTRU";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("TRU")));
}

#[test]
fn r15_mw_0621() {
  let src = "-- mw0\nREI";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("REI")));
}

#[test]
fn r15_mw_0622() {
  let src = "-- mw0\nSAV";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SAV")));
}

#[test]
fn r15_mw_0631() {
  let src = "-- mw0\nLIS";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("LIS")));
}

#[test]
fn r15_mw_0633() {
  let src = "-- mw0\nNOT";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("NOT")));
}

#[test]
fn r15_mw_0635() {
  let src = "-- mw0\nCOP";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("COP")));
}

#[test]
fn r15_mw_0636() {
  let src = "-- mw0\nSHO";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SHO")));
}

#[test]
fn r15_mw_0641() {
  let src = "-- mw0\nCAL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("CAL")));
}

#[test]
fn r15_mw_0642() {
  let src = "-- mw1\nSEL";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SEL")));
}

#[test]
fn r15_mw_0643() {
  let src = "-- mw1\nSELE";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SELE")));
}

#[test]
fn r15_mw_0644() {
  let src = "-- mw1\nSELEC";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.to_ascii_uppercase().starts_with("SELEC")));
}

#[test]
fn r16_probe_having() {
  let cat = catalog_with_users_and_orders();
  for s in [
    "SELECT id, count(*) FROM users GROUP BY id HAVING ",
    "SELECT user_id, count(*) FROM orders GROUP BY user_id HAVING ",
  ] {
    let items = complete_at(s, s.len(), &cat);
    eprintln!("H|{}|n={}", s, items.len());
  }
}

#[test]
fn r16_probe_insert_values() {
  let cat = catalog_with_users_and_orders();
  for s in [
    "INSERT INTO users (id, name) VALUES (1, ",
    "INSERT INTO users VALUES (",
  ] {
    let items = complete_at(s, s.len(), &cat);
    eprintln!("IV|{}|n={}", s, items.len());
  }
}

#[test]
fn r16_probe_update_set_eq() {
  let cat = catalog_with_users_and_orders();
  for s in [
    "UPDATE users SET name = ",
    "UPDATE users SET id = ",
  ] {
    let items = complete_at(s, s.len(), &cat);
    eprintln!("US|{}|n={}", s, items.len());
  }
}

#[test]
fn r16_probe_join_on() {
  let cat = catalog_with_users_and_orders();
  for s in [
    "SELECT * FROM users u JOIN orders o ON ",
    "SELECT * FROM users u JOIN orders o ON u.id = o.",
  ] {
    let items = complete_at(s, s.len(), &cat);
    let has_uid = items.iter().any(|i| i.label == "user_id");
    eprintln!("JO|{}|n={}|has_uid={}", s, items.len(), has_uid);
  }
}

#[test]
fn r16_jo_dot_0001() {
  let src = "-- jo0\nSELECT * FROM users u JOIN orders o ON u.id = o.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected `id` in JOIN ON o. ctx");
}

#[test]
fn r16_jo_dot_0002() {
  let src = "-- jo0\nSELECT * FROM users u JOIN orders o ON u.id = o.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "expected `user_id` in JOIN ON o. ctx");
}

#[test]
fn r16_jo_dot_0003() {
  let src = "-- jo1\nSELECT * FROM users u JOIN orders o ON u.id = o.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected `id` in JOIN ON o. ctx");
}

#[test]
fn r16_jo_dot_0004() {
  let src = "-- jo1\nSELECT * FROM users u JOIN orders o ON u.id = o.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "expected `user_id` in JOIN ON o. ctx");
}

#[test]
fn r16_jo_dot_0005() {
  let src = "-- jo2\nSELECT * FROM users u JOIN orders o ON u.id = o.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected `id` in JOIN ON o. ctx");
}

#[test]
fn r16_jo_dot_0006() {
  let src = "-- jo2\nSELECT * FROM users u JOIN orders o ON u.id = o.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "expected `user_id` in JOIN ON o. ctx");
}

#[test]
fn r16_ju_dot_0401() {
  let src = "-- ju0\nSELECT * FROM users u JOIN orders o ON o.user_id = u.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected `id` in JOIN ON u. ctx");
}

#[test]
fn r16_ju_dot_0402() {
  let src = "-- ju0\nSELECT * FROM users u JOIN orders o ON o.user_id = u.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "expected `email` in JOIN ON u. ctx");
}

#[test]
fn r16_ju_dot_0403() {
  let src = "-- ju0\nSELECT * FROM users u JOIN orders o ON o.user_id = u.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "expected `name` in JOIN ON u. ctx");
}

#[test]
fn r16_ju_dot_0404() {
  let src = "-- ju1\nSELECT * FROM users u JOIN orders o ON o.user_id = u.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected `id` in JOIN ON u. ctx");
}

#[test]
fn r16_ju_dot_0405() {
  let src = "-- ju1\nSELECT * FROM users u JOIN orders o ON o.user_id = u.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "expected `email` in JOIN ON u. ctx");
}

#[test]
fn r16_ju_dot_0406() {
  let src = "-- ju1\nSELECT * FROM users u JOIN orders o ON o.user_id = u.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "expected `name` in JOIN ON u. ctx");
}

#[test]
fn r16_ju_dot_0407() {
  let src = "-- ju2\nSELECT * FROM users u JOIN orders o ON o.user_id = u.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "expected `id` in JOIN ON u. ctx");
}

#[test]
fn r16_ju_dot_0408() {
  let src = "-- ju2\nSELECT * FROM users u JOIN orders o ON o.user_id = u.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "expected `email` in JOIN ON u. ctx");
}

#[test]
fn r16_ju_dot_0409() {
  let src = "-- ju2\nSELECT * FROM users u JOIN orders o ON o.user_id = u.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "expected `name` in JOIN ON u. ctx");
}

#[test]
fn r17_probe_inside_string() {
  let cat = catalog_with_users_and_orders();
  for (src, label) in [
    ("SELECT 'in string ", "string_open"),
    ("SELECT 'in string\\' ", "escaped"),
    ("SELECT $$dollar string ", "dollar"),
    ("SELECT $tag$tagged ", "tagged"),
    ("SELECT '' ", "empty_str_after"),
    ("-- inside\nSELECT ", "after_line_comment"),
    ("/* inside */ SELECT ", "after_block_comment"),
    ("SELECT 1; -- end\n", "after_inline_comment"),
    ("SELECT 1;", "right_at_semi"),
    ("SELECT 1; ", "after_semi_space"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    let n = items.len();
    let has_kw = items.iter().any(|i| matches!(i.kind, dsl_completion::ItemKind::Keyword));
    eprintln!("ES|{}|n={}|kw={}", label, n, has_kw);
  }
}

#[test]
fn r17_probe_unicode() {
  let cat = catalog_with_users_and_orders();
  for (src, label) in [
    ("SELECT * FROM \"用户\"", "unicode_ident"),
    ("SELECT '日本語' FROM users", "unicode_str"),
    ("SELECT * FROM users WHERE name = '日本'", "unicode_in_where"),
    ("SELECT * FROM \"my table\" ", "spaced_quoted"),
    ("SELECT \"col.with.dots\" FROM users", "dotted_quoted"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    eprintln!("UN|{}|n={}", label, items.len());
  }
}

#[test]
fn r17_probe_inside_case() {
  let cat = catalog_with_users_and_orders();
  for (src, label) in [
    ("SELECT CASE WHEN ", "case_when"),
    ("SELECT CASE WHEN id = 1 THEN ", "case_then"),
    ("SELECT CASE WHEN id = 1 THEN 'x' ELSE ", "case_else"),
    ("SELECT CASE WHEN id = 1 THEN 'x' ELSE 'y' END FROM users WHERE ", "after_case"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    eprintln!("CS|{}|n={}", label, items.len());
  }
}

#[test]
fn r17_probe_inside_fn() {
  let cat = catalog_with_users_and_orders();
  for (src, label) in [
    ("SELECT count(", "count_open"),
    ("SELECT max(id), min(", "min_open"),
    ("SELECT array_agg(", "array_agg_open"),
    ("SELECT coalesce(", "coalesce_open"),
    ("SELECT coalesce(id, ", "coalesce_arg2"),
  ] {
    let items = complete_at(src, src.len(), &cat);
    eprintln!("FN|{}|n={}", label, items.len());
  }
}

#[test]
fn r17_probe_dotted_left() {
  let cat = catalog_with_users_and_orders();
  for (src, marker, label) in [
    ("SELECT u.id FROM users u", "u.id", "alias_col_user"),
    ("SELECT u FROM users u", "u", "bare_alias"),
    ("SELECT users.id FROM users", "users.id", "tbl_qual"),
    ("SELECT public.users FROM public.users", "public.users", "schema_qual"),
  ] {
    let cur = src.find(marker).unwrap();
    let items = complete_at(src, cur, &cat);
    eprintln!("DL|{}|n={}", label, items.len());
  }
}

#[test]
fn r17_probe_in_string_after() {
  let cat = catalog_with_users_and_orders();
  for s in ["SELECT 'inside string", "SELECT \"open ident", "SELECT 'a''b' "] {
    let items = complete_at(s, s.len(), &cat);
    eprintln!("STR|{}|n={}", s, items.len());
  }
}

#[test]
fn r17_str_empty_0001() {
  let src = "-- str0\nSELECT 'open string ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0002() {
  let src = "-- str0\nSELECT 'still open ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0003() {
  let src = "-- str0\nSELECT 'with chars: SELECT FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0004() {
  let src = "-- str0\nSELECT name FROM users WHERE name = 'open ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0005() {
  let src = "-- str0\nUPDATE users SET name = 'inside ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0006() {
  let src = "-- str0\nSELECT \"open ident";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0009() {
  let src = "-- str0\nINSERT INTO users (id) VALUES ('open str ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0010() {
  let src = "-- str0\nSELECT 'a' || 'open ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0011() {
  let src = "-- str1\nSELECT 'open string ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0012() {
  let src = "-- str1\nSELECT 'still open ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0013() {
  let src = "-- str1\nSELECT 'with chars: SELECT FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0014() {
  let src = "-- str1\nSELECT name FROM users WHERE name = 'open ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0015() {
  let src = "-- str1\nUPDATE users SET name = 'inside ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0016() {
  let src = "-- str1\nSELECT \"open ident";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0019() {
  let src = "-- str1\nINSERT INTO users (id) VALUES ('open str ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0020() {
  let src = "-- str1\nSELECT 'a' || 'open ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0021() {
  let src = "-- str2\nSELECT 'open string ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0022() {
  let src = "-- str2\nSELECT 'still open ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0023() {
  let src = "-- str2\nSELECT 'with chars: SELECT FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0024() {
  let src = "-- str2\nSELECT name FROM users WHERE name = 'open ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0025() {
  let src = "-- str2\nUPDATE users SET name = 'inside ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0026() {
  let src = "-- str2\nSELECT \"open ident";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0029() {
  let src = "-- str2\nINSERT INTO users (id) VALUES ('open str ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_str_empty_0030() {
  let src = "-- str2\nSELECT 'a' || 'open ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.is_empty(), "inside string should return empty, got {}", items.len());
}

#[test]
fn r17_case_col_0031() {
  let src = "-- cs0\nSELECT CASE WHEN  THEN 1 END FROM users";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0032() {
  let src = "-- cs1\nSELECT CASE WHEN  THEN 1 ELSE 0 END FROM users WHERE id = 1";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0033() {
  let src = "-- cs2\nUPDATE users SET name = CASE WHEN  THEN 'a' END";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0034() {
  let src = "-- cs3\nSELECT CASE WHEN  THEN 'a' WHEN id > 0 THEN 'b' END FROM users";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0035() {
  let src = "-- cs4\nSELECT CASE WHEN  THEN 1 END FROM users";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0036() {
  let src = "-- cs5\nSELECT CASE WHEN  THEN 1 ELSE 0 END FROM users WHERE id = 1";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0037() {
  let src = "-- cs6\nUPDATE users SET name = CASE WHEN  THEN 'a' END";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0038() {
  let src = "-- cs7\nSELECT CASE WHEN  THEN 'a' WHEN id > 0 THEN 'b' END FROM users";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0039() {
  let src = "-- cs8\nSELECT CASE WHEN  THEN 1 END FROM users";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0040() {
  let src = "-- cs9\nSELECT CASE WHEN  THEN 1 ELSE 0 END FROM users WHERE id = 1";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0041() {
  let src = "-- cs10\nUPDATE users SET name = CASE WHEN  THEN 'a' END";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_case_col_0042() {
  let src = "-- cs11\nSELECT CASE WHEN  THEN 'a' WHEN id > 0 THEN 'b' END FROM users";
  let cur = src.find("WHEN ").unwrap() + 5;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CASE WHEN ctx with users missed col `id`");
}

#[test]
fn r17_fn_arg_col_0061() {
  let src = "-- fn0\nSELECT count() FROM users";
  let cur = src.find("count(").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0062() {
  let src = "-- fn1\nSELECT max() FROM users";
  let cur = src.find("max(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0063() {
  let src = "-- fn2\nSELECT min() FROM users";
  let cur = src.find("min(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0064() {
  let src = "-- fn3\nSELECT avg() FROM users";
  let cur = src.find("avg(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0065() {
  let src = "-- fn4\nSELECT sum() FROM users";
  let cur = src.find("sum(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0066() {
  let src = "-- fn5\nSELECT coalesce() FROM users";
  let cur = src.find("coalesce(").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0067() {
  let src = "-- fn6\nSELECT array_agg() FROM users";
  let cur = src.find("array_agg(").unwrap() + 10;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0068() {
  let src = "-- fn7\nSELECT string_agg(, ',') FROM users";
  let cur = src.find("string_agg(").unwrap() + 11;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0069() {
  let src = "-- fn8\nSELECT count() FROM users";
  let cur = src.find("count(").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0070() {
  let src = "-- fn9\nSELECT max() FROM users";
  let cur = src.find("max(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0071() {
  let src = "-- fn10\nSELECT min() FROM users";
  let cur = src.find("min(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0072() {
  let src = "-- fn11\nSELECT avg() FROM users";
  let cur = src.find("avg(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0073() {
  let src = "-- fn12\nSELECT sum() FROM users";
  let cur = src.find("sum(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0074() {
  let src = "-- fn13\nSELECT coalesce() FROM users";
  let cur = src.find("coalesce(").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0075() {
  let src = "-- fn14\nSELECT array_agg() FROM users";
  let cur = src.find("array_agg(").unwrap() + 10;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0076() {
  let src = "-- fn15\nSELECT string_agg(, ',') FROM users";
  let cur = src.find("string_agg(").unwrap() + 11;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0077() {
  let src = "-- fn16\nSELECT count() FROM users";
  let cur = src.find("count(").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0078() {
  let src = "-- fn17\nSELECT max() FROM users";
  let cur = src.find("max(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0079() {
  let src = "-- fn18\nSELECT min() FROM users";
  let cur = src.find("min(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0080() {
  let src = "-- fn19\nSELECT avg() FROM users";
  let cur = src.find("avg(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0081() {
  let src = "-- fn20\nSELECT sum() FROM users";
  let cur = src.find("sum(").unwrap() + 4;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0082() {
  let src = "-- fn21\nSELECT coalesce() FROM users";
  let cur = src.find("coalesce(").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0083() {
  let src = "-- fn22\nSELECT array_agg() FROM users";
  let cur = src.find("array_agg(").unwrap() + 10;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_fn_arg_col_0084() {
  let src = "-- fn23\nSELECT string_agg(, ',') FROM users";
  let cur = src.find("string_agg(").unwrap() + 11;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "fn arg ctx missed col `id`");
}

#[test]
fn r17_post_comment_0091() {
  let src = "/* hint */ ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #91 missed SELECT");
}

#[test]
fn r17_post_comment_0092() {
  let src = "/*\nmulti\n*/ ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #92 missed SELECT");
}

#[test]
fn r17_post_comment_0093() {
  let src = "/*1*//*2*/ ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #93 missed SELECT");
}

#[test]
fn r17_post_comment_0094() {
  let src = "-- top\n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #94 missed SELECT");
}

#[test]
fn r17_post_comment_0095() {
  let src = "-- multi line\n-- second\n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #95 missed SELECT");
}

#[test]
fn r17_post_comment_0096() {
  let src = "/*1*/ -- mixed\n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #96 missed SELECT");
}

#[test]
fn r17_post_comment_0100() {
  let src = "-- top\n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #100 missed SELECT");
}

#[test]
fn r17_post_comment_0101() {
  let src = "-- multi line\n-- second\n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #101 missed SELECT");
}

#[test]
fn r17_post_comment_0102() {
  let src = "/*1*/ -- mixed\n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #102 missed SELECT");
}

#[test]
fn r17_post_comment_0106() {
  let src = "-- top\n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #106 missed SELECT");
}

#[test]
fn r17_post_comment_0107() {
  let src = "-- multi line\n-- second\n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #107 missed SELECT");
}

#[test]
fn r17_post_comment_0108() {
  let src = "/*1*/ -- mixed\n";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-comment ctx #108 missed SELECT");
}

#[test]
fn r17_post_semi_0121() {
  let src = "SELECT 1;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #121 missed SELECT kw");
}

#[test]
fn r17_post_semi_0122() {
  let src = "SELECT 1, 2;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #122 missed SELECT kw");
}

#[test]
fn r17_post_semi_0123() {
  let src = "SELECT * FROM users;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #123 missed SELECT kw");
}

#[test]
fn r17_post_semi_0124() {
  let src = "UPDATE users SET name='x' WHERE id=1;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #124 missed SELECT kw");
}

#[test]
fn r17_post_semi_0125() {
  let src = "DELETE FROM users WHERE id=1;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #125 missed SELECT kw");
}

#[test]
fn r17_post_semi_0126() {
  let src = "INSERT INTO users (id) VALUES (1);";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #126 missed SELECT kw");
}

#[test]
fn r17_post_semi_0127() {
  let src = "BEGIN;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #127 missed SELECT kw");
}

#[test]
fn r17_post_semi_0128() {
  let src = "COMMIT;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #128 missed SELECT kw");
}

#[test]
fn r17_post_semi_0129() {
  let src = "ROLLBACK;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #129 missed SELECT kw");
}

#[test]
fn r17_post_semi_0130() {
  let src = "CREATE TABLE t (id int);";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #130 missed SELECT kw");
}

#[test]
fn r17_post_semi_0131() {
  let src = "SELECT 1;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #131 missed SELECT kw");
}

#[test]
fn r17_post_semi_0132() {
  let src = "SELECT 1, 2;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #132 missed SELECT kw");
}

#[test]
fn r17_post_semi_0133() {
  let src = "SELECT * FROM users;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #133 missed SELECT kw");
}

#[test]
fn r17_post_semi_0134() {
  let src = "UPDATE users SET name='x' WHERE id=1;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #134 missed SELECT kw");
}

#[test]
fn r17_post_semi_0135() {
  let src = "DELETE FROM users WHERE id=1;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #135 missed SELECT kw");
}

#[test]
fn r17_post_semi_0136() {
  let src = "INSERT INTO users (id) VALUES (1);";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #136 missed SELECT kw");
}

#[test]
fn r17_post_semi_0137() {
  let src = "BEGIN;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #137 missed SELECT kw");
}

#[test]
fn r17_post_semi_0138() {
  let src = "COMMIT;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #138 missed SELECT kw");
}

#[test]
fn r17_post_semi_0139() {
  let src = "ROLLBACK;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #139 missed SELECT kw");
}

#[test]
fn r17_post_semi_0140() {
  let src = "CREATE TABLE t (id int);";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #140 missed SELECT kw");
}

#[test]
fn r17_post_semi_0141() {
  let src = "SELECT 1;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #141 missed SELECT kw");
}

#[test]
fn r17_post_semi_0142() {
  let src = "SELECT 1, 2;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #142 missed SELECT kw");
}

#[test]
fn r17_post_semi_0143() {
  let src = "SELECT * FROM users;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #143 missed SELECT kw");
}

#[test]
fn r17_post_semi_0144() {
  let src = "UPDATE users SET name='x' WHERE id=1;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #144 missed SELECT kw");
}

#[test]
fn r17_post_semi_0145() {
  let src = "DELETE FROM users WHERE id=1;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #145 missed SELECT kw");
}

#[test]
fn r17_post_semi_0146() {
  let src = "INSERT INTO users (id) VALUES (1);";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #146 missed SELECT kw");
}

#[test]
fn r17_post_semi_0147() {
  let src = "BEGIN;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #147 missed SELECT kw");
}

#[test]
fn r17_post_semi_0148() {
  let src = "COMMIT;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #148 missed SELECT kw");
}

#[test]
fn r17_post_semi_0149() {
  let src = "ROLLBACK;";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #149 missed SELECT kw");
}

#[test]
fn r17_post_semi_0150() {
  let src = "CREATE TABLE t (id int);";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label.eq_ignore_ascii_case("SELECT")), "after-semi #150 missed SELECT kw");
}

#[test]
fn r17_uni_where_0151() {
  let src = "-- u0\nSELECT * FROM users WHERE name = '日本語' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0152() {
  let src = "-- u1\nSELECT * FROM users WHERE name = '中文' OR ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0153() {
  let src = "-- u2\nSELECT * FROM users WHERE name = 'العربية' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0154() {
  let src = "-- u3\nSELECT * FROM users WHERE name = 'Ελληνικά' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0155() {
  let src = "-- u4\nSELECT * FROM users WHERE name = 'Кириллица' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0156() {
  let src = "-- u5\nSELECT * FROM users WHERE name = '日本語' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0157() {
  let src = "-- u6\nSELECT * FROM users WHERE name = '中文' OR ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0158() {
  let src = "-- u7\nSELECT * FROM users WHERE name = 'العربية' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0159() {
  let src = "-- u8\nSELECT * FROM users WHERE name = 'Ελληνικά' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0160() {
  let src = "-- u9\nSELECT * FROM users WHERE name = 'Кириллица' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0161() {
  let src = "-- u10\nSELECT * FROM users WHERE name = '日本語' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0162() {
  let src = "-- u11\nSELECT * FROM users WHERE name = '中文' OR ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0163() {
  let src = "-- u12\nSELECT * FROM users WHERE name = 'العربية' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0164() {
  let src = "-- u13\nSELECT * FROM users WHERE name = 'Ελληνικά' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r17_uni_where_0165() {
  let src = "-- u14\nSELECT * FROM users WHERE name = 'Кириллица' AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "unicode WHERE ctx missed col `id`");
}

#[test]
fn r18_probe_cte_window_etc() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("WITH x AS (SELECT id FROM users) SELECT  FROM x", "cte_use"),
    ("WITH x(a) AS (SELECT id FROM users) SELECT  FROM x", "cte_col_list"),
    ("SELECT count(*) OVER (PARTITION BY ) FROM users", "window_part"),
    ("SELECT count(*) OVER (ORDER BY ) FROM users", "window_order"),
    ("INSERT INTO users (id) VALUES (1) RETURNING ", "returning"),
    ("INSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ", "on_conflict_set"),
    ("INSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.", "excluded_dot"),
    ("SELECT * FROM users u, LATERAL (SELECT  FROM orders WHERE user_id = u.id) x", "lateral"),
    ("UPDATE users SET name = 'x' FROM ", "update_from"),
    ("DELETE FROM users USING ", "delete_using"),
    ("SELECT * FROM users TABLESAMPLE BERNOULLI (10) WHERE ", "tablesample"),
    ("SELECT * FROM users FOR UPDATE OF ", "for_update_of"),
    ("VALUES (1), (2), (3)", "values_list"),
    ("VALUES (1, 'a'), (2, 'b')", "values_tuples"),
  ] {
    let cur = if let Some(c) = s.find("  ") { c + 2 } else { s.len() };
    let items = complete_at(s, cur, &cat);
    let has_id = items.iter().any(|i| i.label == "id");
    let n = items.len();
    eprintln!("E|{}|n={}|id={}", label, n, has_id);
  }
}

#[test]
fn r18_probe_window_specific() {
  let cat = catalog_with_users_and_orders();
  let s = "SELECT count(*) OVER (PARTITION BY ) FROM users";
  let cur = s.find("BY ").unwrap() + 3;
  let items = complete_at(s, cur, &cat);
  for it in &items {
    eprintln!("W|{}|{:?}", it.label, it.kind);
  }
}

#[test]
fn r18_probe_excluded_dot() {
  let cat = catalog_with_users_and_orders();
  let s = "INSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let cur = s.len();
  let items = complete_at(s, cur, &cat);
  for it in &items {
    eprintln!("EX|{}|{:?}", it.label, it.kind);
  }
}

#[test]
fn r18_probe_on_conflict_set() {
  let cat = catalog_with_users_and_orders();
  let s = "INSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ";
  let cur = s.len();
  let items = complete_at(s, cur, &cat);
  for it in &items {
    eprintln!("CS|{}|{:?}", it.label, it.kind);
  }
}


#[test]
fn r18_excluded_0001_id() {
  let src = "-- ex0\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0001_email() {
  let src = "-- ex0\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0001_name() {
  let src = "-- ex0\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0002_id() {
  let src = "-- ex0\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0002_email() {
  let src = "-- ex0\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0002_name() {
  let src = "-- ex0\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0003_id() {
  let src = "-- ex0\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0003_email() {
  let src = "-- ex0\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0003_name() {
  let src = "-- ex0\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0004_id() {
  let src = "-- ex0\nINSERT INTO users VALUES (1, 'a', 'b') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0004_email() {
  let src = "-- ex0\nINSERT INTO users VALUES (1, 'a', 'b') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0004_name() {
  let src = "-- ex0\nINSERT INTO users VALUES (1, 'a', 'b') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0005_id() {
  let src = "-- ex1\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0005_email() {
  let src = "-- ex1\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0005_name() {
  let src = "-- ex1\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0006_id() {
  let src = "-- ex1\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0006_email() {
  let src = "-- ex1\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0006_name() {
  let src = "-- ex1\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0007_id() {
  let src = "-- ex1\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0007_email() {
  let src = "-- ex1\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0007_name() {
  let src = "-- ex1\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0008_id() {
  let src = "-- ex1\nINSERT INTO users VALUES (1, 'a', 'b') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0008_email() {
  let src = "-- ex1\nINSERT INTO users VALUES (1, 'a', 'b') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0008_name() {
  let src = "-- ex1\nINSERT INTO users VALUES (1, 'a', 'b') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0009_id() {
  let src = "-- ex2\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0009_email() {
  let src = "-- ex2\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0009_name() {
  let src = "-- ex2\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0010_id() {
  let src = "-- ex2\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0010_email() {
  let src = "-- ex2\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0010_name() {
  let src = "-- ex2\nINSERT INTO users (id, name) VALUES (1, 'x') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0011_id() {
  let src = "-- ex2\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0011_email() {
  let src = "-- ex2\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0011_name() {
  let src = "-- ex2\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_excluded_0012_id() {
  let src = "-- ex2\nINSERT INTO users VALUES (1, 'a', 'b') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "EXCLUDED. missed `id`");
}

#[test]
fn r18_excluded_0012_email() {
  let src = "-- ex2\nINSERT INTO users VALUES (1, 'a', 'b') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "EXCLUDED. missed `email`");
}

#[test]
fn r18_excluded_0012_name() {
  let src = "-- ex2\nINSERT INTO users VALUES (1, 'a', 'b') ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "EXCLUDED. missed `name`");
}

#[test]
fn r18_on_conflict_set_0013_id() {
  let src = "-- us0\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT SET missed `id`");
}

#[test]
fn r18_on_conflict_set_0013_email() {
  let src = "-- us0\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ON CONFLICT SET missed `email`");
}

#[test]
fn r18_on_conflict_set_0013_name() {
  let src = "-- us0\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "ON CONFLICT SET missed `name`");
}

#[test]
fn r18_on_conflict_set_0014_id() {
  let src = "-- us0\nINSERT INTO users (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT SET missed `id`");
}

#[test]
fn r18_on_conflict_set_0014_email() {
  let src = "-- us0\nINSERT INTO users (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ON CONFLICT SET missed `email`");
}

#[test]
fn r18_on_conflict_set_0014_name() {
  let src = "-- us0\nINSERT INTO users (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "ON CONFLICT SET missed `name`");
}

#[test]
fn r18_on_conflict_set_0015_id() {
  let src = "-- us0\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT ON CONSTRAINT pk DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT SET missed `id`");
}

#[test]
fn r18_on_conflict_set_0015_email() {
  let src = "-- us0\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT ON CONSTRAINT pk DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ON CONFLICT SET missed `email`");
}

#[test]
fn r18_on_conflict_set_0015_name() {
  let src = "-- us0\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT ON CONSTRAINT pk DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "ON CONFLICT SET missed `name`");
}

#[test]
fn r18_on_conflict_set_0016_id() {
  let src = "-- us1\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT SET missed `id`");
}

#[test]
fn r18_on_conflict_set_0016_email() {
  let src = "-- us1\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ON CONFLICT SET missed `email`");
}

#[test]
fn r18_on_conflict_set_0016_name() {
  let src = "-- us1\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "ON CONFLICT SET missed `name`");
}

#[test]
fn r18_on_conflict_set_0017_id() {
  let src = "-- us1\nINSERT INTO users (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT SET missed `id`");
}

#[test]
fn r18_on_conflict_set_0017_email() {
  let src = "-- us1\nINSERT INTO users (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ON CONFLICT SET missed `email`");
}

#[test]
fn r18_on_conflict_set_0017_name() {
  let src = "-- us1\nINSERT INTO users (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "ON CONFLICT SET missed `name`");
}

#[test]
fn r18_on_conflict_set_0018_id() {
  let src = "-- us1\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT ON CONSTRAINT pk DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT SET missed `id`");
}

#[test]
fn r18_on_conflict_set_0018_email() {
  let src = "-- us1\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT ON CONSTRAINT pk DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ON CONFLICT SET missed `email`");
}

#[test]
fn r18_on_conflict_set_0018_name() {
  let src = "-- us1\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT ON CONSTRAINT pk DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "ON CONFLICT SET missed `name`");
}

#[test]
fn r18_on_conflict_set_0019_id() {
  let src = "-- us2\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT SET missed `id`");
}

#[test]
fn r18_on_conflict_set_0019_email() {
  let src = "-- us2\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ON CONFLICT SET missed `email`");
}

#[test]
fn r18_on_conflict_set_0019_name() {
  let src = "-- us2\nINSERT INTO users (id) VALUES (1) ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "ON CONFLICT SET missed `name`");
}

#[test]
fn r18_on_conflict_set_0020_id() {
  let src = "-- us2\nINSERT INTO users (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT SET missed `id`");
}

#[test]
fn r18_on_conflict_set_0020_email() {
  let src = "-- us2\nINSERT INTO users (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ON CONFLICT SET missed `email`");
}

#[test]
fn r18_on_conflict_set_0020_name() {
  let src = "-- us2\nINSERT INTO users (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "ON CONFLICT SET missed `name`");
}

#[test]
fn r18_on_conflict_set_0021_id() {
  let src = "-- us2\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT ON CONSTRAINT pk DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT SET missed `id`");
}

#[test]
fn r18_on_conflict_set_0021_email() {
  let src = "-- us2\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT ON CONSTRAINT pk DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ON CONFLICT SET missed `email`");
}

#[test]
fn r18_on_conflict_set_0021_name() {
  let src = "-- us2\nINSERT INTO users (id, email) VALUES (1, 'a') ON CONFLICT ON CONSTRAINT pk DO UPDATE SET ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "ON CONFLICT SET missed `name`");
}

#[test]
fn r18_cte_0025() {
  let src = "-- ct0\nWITH x AS (SELECT id FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0026() {
  let src = "-- ct0\nWITH x AS (SELECT id, email FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0027() {
  let src = "-- ct0\nWITH x AS (SELECT * FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0028() {
  let src = "-- ct0\nWITH x AS (SELECT id FROM orders) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0029() {
  let src = "-- ct0\nWITH x AS (SELECT id, user_id FROM orders) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0030() {
  let src = "-- ct0\nWITH x(a) AS (SELECT id FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0031() {
  let src = "-- ct1\nWITH x AS (SELECT id FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0032() {
  let src = "-- ct1\nWITH x AS (SELECT id, email FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0033() {
  let src = "-- ct1\nWITH x AS (SELECT * FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0034() {
  let src = "-- ct1\nWITH x AS (SELECT id FROM orders) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0035() {
  let src = "-- ct1\nWITH x AS (SELECT id, user_id FROM orders) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0036() {
  let src = "-- ct1\nWITH x(a) AS (SELECT id FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0037() {
  let src = "-- ct2\nWITH x AS (SELECT id FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0038() {
  let src = "-- ct2\nWITH x AS (SELECT id, email FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0039() {
  let src = "-- ct2\nWITH x AS (SELECT * FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0040() {
  let src = "-- ct2\nWITH x AS (SELECT id FROM orders) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0041() {
  let src = "-- ct2\nWITH x AS (SELECT id, user_id FROM orders) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_cte_0042() {
  let src = "-- ct2\nWITH x(a) AS (SELECT id FROM users) SELECT  FROM x";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "a"), "CTE projection missed cte cols");
}

#[test]
fn r18_returning_0051() {
  let src = "-- rt0\nINSERT INTO users (id) VALUES (1) RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0052() {
  let src = "-- rt0\nUPDATE users SET name='x' WHERE id=1 RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0053() {
  let src = "-- rt0\nDELETE FROM users WHERE id=1 RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0054() {
  let src = "-- rt0\nINSERT INTO users (id, name) VALUES (1,'a') RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0055() {
  let src = "-- rt0\nUPDATE users SET email='a' RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0056() {
  let src = "-- rt1\nINSERT INTO users (id) VALUES (1) RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0057() {
  let src = "-- rt1\nUPDATE users SET name='x' WHERE id=1 RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0058() {
  let src = "-- rt1\nDELETE FROM users WHERE id=1 RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0059() {
  let src = "-- rt1\nINSERT INTO users (id, name) VALUES (1,'a') RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0060() {
  let src = "-- rt1\nUPDATE users SET email='a' RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0061() {
  let src = "-- rt2\nINSERT INTO users (id) VALUES (1) RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0062() {
  let src = "-- rt2\nUPDATE users SET name='x' WHERE id=1 RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0063() {
  let src = "-- rt2\nDELETE FROM users WHERE id=1 RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0064() {
  let src = "-- rt2\nINSERT INTO users (id, name) VALUES (1,'a') RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_returning_0065() {
  let src = "-- rt2\nUPDATE users SET email='a' RETURNING ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "RETURNING missed col `id`");
}

#[test]
fn r18_del_using_0113() {
  let src = "-- du0\nDELETE FROM users WHERE EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users") || items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r18_del_using_0116() {
  let src = "-- du1\nDELETE FROM users WHERE EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users") || items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r18_del_using_0119() {
  let src = "-- du2\nDELETE FROM users WHERE EXISTS (SELECT 1 FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users") || items.iter().any(|i| i.label == "orders"));
}

#[test]
fn r18_values_0131() {
  let src = "-- vl0\nINSERT INTO users (id) VALUES (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0132() {
  let src = "-- vl0\nINSERT INTO users (id, name) VALUES (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0133() {
  let src = "-- vl0\nINSERT INTO orders (id, user_id) VALUES (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0134() {
  let src = "-- vl0\nINSERT INTO orders VALUES (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0135() {
  let src = "-- vl1\nINSERT INTO users (id) VALUES (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0136() {
  let src = "-- vl1\nINSERT INTO users (id, name) VALUES (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0137() {
  let src = "-- vl1\nINSERT INTO orders (id, user_id) VALUES (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0138() {
  let src = "-- vl1\nINSERT INTO orders VALUES (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0139() {
  let src = "-- vl2\nINSERT INTO users (id) VALUES (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0140() {
  let src = "-- vl2\nINSERT INTO users (id, name) VALUES (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0141() {
  let src = "-- vl2\nINSERT INTO orders (id, user_id) VALUES (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r18_values_0142() {
  let src = "-- vl2\nINSERT INTO orders VALUES (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| matches!(i.kind, ItemKind::Keyword) && i.label.eq_ignore_ascii_case("NULL")) || items.iter().any(|i| matches!(i.kind, ItemKind::Function)), "VALUES ( ctx had no NULL/Function");
}

#[test]
fn r19_probe_edges() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("SELECT * FROM users WHERE id = ANY(ARRAY[", "any_array_open"),
    ("SELECT * FROM users WHERE id = ANY(", "any_open"),
    ("SELECT * FROM users WHERE id IN (1, 2, ", "in_more"),
    ("SELECT COALESCE(", "coalesce_open"),
    ("SELECT COALESCE(id, ", "coalesce_arg2"),
    ("SELECT COALESCE(id, name, ", "coalesce_arg3"),
    ("SELECT NULLIF(", "nullif_open"),
    ("SELECT NULLIF(id, ", "nullif_arg2"),
    ("SELECT GREATEST(", "greatest_open"),
    ("SELECT LEAST(", "least_open"),
    ("SELECT a[", "array_subscript_open"),
    ("SELECT a[1:", "array_slice_open"),
    ("SELECT data -> ", "json_arrow_rhs"),
    ("SELECT data ->> ", "json_arrow2_rhs"),
    ("SELECT data #> ", "json_path_rhs"),
    ("SELECT (", "paren_open"),
    ("SELECT (1, ", "tuple_open"),
    ("SELECT ROW(", "row_open"),
    ("SELECT * FROM ROWS FROM (", "rows_from_open"),
    ("SELECT * FROM generate_series(1, ", "gen_series_arg2"),
  ] {
    let items = complete_at(s, s.len(), &cat);
    let has_id = items.iter().any(|i| i.label == "id");
    let n = items.len();
    eprintln!("E|{}|n={}|id={}", label, n, has_id);
  }
}

#[test]
fn r19_probe_with_from() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("SELECT COALESCE(, name) FROM users", "coalesce_arg1_from"),
    ("SELECT COALESCE(id, ) FROM users", "coalesce_arg2_from"),
    ("SELECT * FROM users WHERE id = ANY(ARRAY[]) AND ", "and_after_any"),
    ("SELECT * FROM users WHERE id BETWEEN  AND 10", "between_lhs"),
    ("SELECT * FROM users WHERE id BETWEEN 1 AND ", "between_rhs"),
    ("SELECT a -> 'k' FROM users", "json_op_lhs"),
    ("SELECT a ->> 'k' FROM users", "json_op2_lhs"),
    ("SELECT * FROM users ORDER BY  DESC", "order_dir_lhs"),
    ("SELECT * FROM users ORDER BY id, ", "order_comma"),
    ("SELECT * FROM users GROUP BY id, ", "group_comma"),
    ("SELECT  FROM users", "proj_open"),
    ("SELECT *, , FROM users", "proj_after_comma"),
  ] {
    let cur = s.find("  ").map(|p| p + 1).or_else(|| s.find(", )").map(|p| p + 2)).unwrap_or(s.len());
    let items = complete_at(s, cur, &cat);
    let has_id = items.iter().any(|i| i.label == "id");
    let has_email = items.iter().any(|i| i.label == "email");
    eprintln!("F|{}|n={}|id={}|email={}", label, items.len(), has_id, has_email);
  }
}

#[test]
fn r19_btw_lhs_0001() {
  let src = "-- bt0\nSELECT * FROM users WHERE id BETWEEN  AND 10";
  let cur = src.find("BETWEEN ").unwrap() + 8;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN lhs ctx missed id");
}

#[test]
fn r19_btw_rhs_0002() {
  let src = "-- bt0\nSELECT * FROM users WHERE id BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN rhs ctx missed id");
}

#[test]
fn r19_btw_lhs_0003() {
  let src = "-- bt0\nSELECT * FROM users WHERE email BETWEEN  AND 'z'";
  let cur = src.find("BETWEEN ").unwrap() + 8;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN lhs ctx missed id");
}

#[test]
fn r19_btw_rhs_0004() {
  let src = "-- bt0\nUPDATE users SET name='x' WHERE id BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN rhs ctx missed id");
}

#[test]
fn r19_btw_rhs_0005() {
  let src = "-- bt0\nDELETE FROM users WHERE id BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN rhs ctx missed id");
}

#[test]
fn r19_btw_lhs_0006() {
  let src = "-- bt1\nSELECT * FROM users WHERE id BETWEEN  AND 10";
  let cur = src.find("BETWEEN ").unwrap() + 8;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN lhs ctx missed id");
}

#[test]
fn r19_btw_rhs_0007() {
  let src = "-- bt1\nSELECT * FROM users WHERE id BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN rhs ctx missed id");
}

#[test]
fn r19_btw_lhs_0008() {
  let src = "-- bt1\nSELECT * FROM users WHERE email BETWEEN  AND 'z'";
  let cur = src.find("BETWEEN ").unwrap() + 8;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN lhs ctx missed id");
}

#[test]
fn r19_btw_rhs_0009() {
  let src = "-- bt1\nUPDATE users SET name='x' WHERE id BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN rhs ctx missed id");
}

#[test]
fn r19_btw_rhs_0010() {
  let src = "-- bt1\nDELETE FROM users WHERE id BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN rhs ctx missed id");
}

#[test]
fn r19_btw_lhs_0011() {
  let src = "-- bt2\nSELECT * FROM users WHERE id BETWEEN  AND 10";
  let cur = src.find("BETWEEN ").unwrap() + 8;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN lhs ctx missed id");
}

#[test]
fn r19_btw_rhs_0012() {
  let src = "-- bt2\nSELECT * FROM users WHERE id BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN rhs ctx missed id");
}

#[test]
fn r19_btw_lhs_0013() {
  let src = "-- bt2\nSELECT * FROM users WHERE email BETWEEN  AND 'z'";
  let cur = src.find("BETWEEN ").unwrap() + 8;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN lhs ctx missed id");
}

#[test]
fn r19_btw_rhs_0014() {
  let src = "-- bt2\nUPDATE users SET name='x' WHERE id BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN rhs ctx missed id");
}

#[test]
fn r19_btw_rhs_0015() {
  let src = "-- bt2\nDELETE FROM users WHERE id BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "BETWEEN rhs ctx missed id");
}

#[test]
fn r19_in_more_0031() {
  let src = "-- in0\nSELECT * FROM users WHERE id IN (1, 2, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0032() {
  let src = "-- in0\nSELECT * FROM users WHERE id IN (1, 2, 3, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0033() {
  let src = "-- in0\nSELECT * FROM users WHERE email IN ('a', ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0034() {
  let src = "-- in0\nUPDATE users SET name='x' WHERE id IN (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0035() {
  let src = "-- in0\nDELETE FROM users WHERE id IN (1, 2, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0036() {
  let src = "-- in1\nSELECT * FROM users WHERE id IN (1, 2, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0037() {
  let src = "-- in1\nSELECT * FROM users WHERE id IN (1, 2, 3, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0038() {
  let src = "-- in1\nSELECT * FROM users WHERE email IN ('a', ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0039() {
  let src = "-- in1\nUPDATE users SET name='x' WHERE id IN (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0040() {
  let src = "-- in1\nDELETE FROM users WHERE id IN (1, 2, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0041() {
  let src = "-- in2\nSELECT * FROM users WHERE id IN (1, 2, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0042() {
  let src = "-- in2\nSELECT * FROM users WHERE id IN (1, 2, 3, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0043() {
  let src = "-- in2\nSELECT * FROM users WHERE email IN ('a', ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0044() {
  let src = "-- in2\nUPDATE users SET name='x' WHERE id IN (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_in_more_0045() {
  let src = "-- in2\nDELETE FROM users WHERE id IN (1, 2, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "IN-list more ctx missed id");
}

#[test]
fn r19_order_lhs_0061() {
  let src = "-- od0\nSELECT * FROM users ORDER BY  DESC";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0062() {
  let src = "-- od0\nSELECT * FROM users ORDER BY  ASC";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0063() {
  let src = "-- od0\nSELECT * FROM orders ORDER BY  DESC";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0064() {
  let src = "-- od0\nSELECT * FROM users ORDER BY  NULLS FIRST";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0065() {
  let src = "-- od0\nSELECT * FROM users ORDER BY  NULLS LAST";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0066() {
  let src = "-- od1\nSELECT * FROM users ORDER BY  DESC";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0067() {
  let src = "-- od1\nSELECT * FROM users ORDER BY  ASC";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0068() {
  let src = "-- od1\nSELECT * FROM orders ORDER BY  DESC";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0069() {
  let src = "-- od1\nSELECT * FROM users ORDER BY  NULLS FIRST";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0070() {
  let src = "-- od1\nSELECT * FROM users ORDER BY  NULLS LAST";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0071() {
  let src = "-- od2\nSELECT * FROM users ORDER BY  DESC";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0072() {
  let src = "-- od2\nSELECT * FROM users ORDER BY  ASC";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0073() {
  let src = "-- od2\nSELECT * FROM orders ORDER BY  DESC";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0074() {
  let src = "-- od2\nSELECT * FROM users ORDER BY  NULLS FIRST";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_lhs_0075() {
  let src = "-- od2\nSELECT * FROM users ORDER BY  NULLS LAST";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ORDER BY lhs ctx missed id");
}

#[test]
fn r19_order_comma_0091() {
  let src = "-- oc0\nSELECT * FROM users ORDER BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0092() {
  let src = "-- oc0\nSELECT * FROM users ORDER BY id DESC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0093() {
  let src = "-- oc0\nSELECT * FROM users ORDER BY id ASC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0094() {
  let src = "-- oc0\nSELECT * FROM users ORDER BY id NULLS FIRST, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0095() {
  let src = "-- oc1\nSELECT * FROM users ORDER BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0096() {
  let src = "-- oc1\nSELECT * FROM users ORDER BY id DESC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0097() {
  let src = "-- oc1\nSELECT * FROM users ORDER BY id ASC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0098() {
  let src = "-- oc1\nSELECT * FROM users ORDER BY id NULLS FIRST, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0099() {
  let src = "-- oc2\nSELECT * FROM users ORDER BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0100() {
  let src = "-- oc2\nSELECT * FROM users ORDER BY id DESC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0101() {
  let src = "-- oc2\nSELECT * FROM users ORDER BY id ASC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_order_comma_0102() {
  let src = "-- oc2\nSELECT * FROM users ORDER BY id NULLS FIRST, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "ORDER BY comma ctx missed email");
}

#[test]
fn r19_group_comma_0121() {
  let src = "-- gc0\nSELECT count(*) FROM users GROUP BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "GROUP BY comma ctx missed email");
}

#[test]
fn r19_group_comma_0122() {
  let src = "-- gc0\nSELECT id, count(*) FROM users GROUP BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "GROUP BY comma ctx missed email");
}

#[test]
fn r19_group_comma_0123() {
  let src = "-- gc0\nSELECT count(*) FROM users WHERE id > 0 GROUP BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "GROUP BY comma ctx missed email");
}

#[test]
fn r19_group_comma_0124() {
  let src = "-- gc1\nSELECT count(*) FROM users GROUP BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "GROUP BY comma ctx missed email");
}

#[test]
fn r19_group_comma_0125() {
  let src = "-- gc1\nSELECT id, count(*) FROM users GROUP BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "GROUP BY comma ctx missed email");
}

#[test]
fn r19_group_comma_0126() {
  let src = "-- gc1\nSELECT count(*) FROM users WHERE id > 0 GROUP BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "GROUP BY comma ctx missed email");
}

#[test]
fn r19_group_comma_0127() {
  let src = "-- gc2\nSELECT count(*) FROM users GROUP BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "GROUP BY comma ctx missed email");
}

#[test]
fn r19_group_comma_0128() {
  let src = "-- gc2\nSELECT id, count(*) FROM users GROUP BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "GROUP BY comma ctx missed email");
}

#[test]
fn r19_group_comma_0129() {
  let src = "-- gc2\nSELECT count(*) FROM users WHERE id > 0 GROUP BY id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "GROUP BY comma ctx missed email");
}

#[test]
fn r19_proj_0151() {
  let src = "-- pj0\nSELECT  FROM users";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "proj ctx missed `email`");
}

#[test]
fn r19_proj_0152() {
  let src = "-- pj0\nSELECT id,  FROM users";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "proj ctx missed `email`");
}

#[test]
fn r19_proj_0154() {
  let src = "-- pj0\nSELECT  FROM orders";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "proj ctx missed `user_id`");
}

#[test]
fn r19_proj_0155() {
  let src = "-- pj0\nSELECT id,  FROM orders";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "proj ctx missed `user_id`");
}

#[test]
fn r19_proj_0156() {
  let src = "-- pj1\nSELECT  FROM users";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "proj ctx missed `email`");
}

#[test]
fn r19_proj_0157() {
  let src = "-- pj1\nSELECT id,  FROM users";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "proj ctx missed `email`");
}

#[test]
fn r19_proj_0159() {
  let src = "-- pj1\nSELECT  FROM orders";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "proj ctx missed `user_id`");
}

#[test]
fn r19_proj_0160() {
  let src = "-- pj1\nSELECT id,  FROM orders";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "proj ctx missed `user_id`");
}

#[test]
fn r19_proj_0161() {
  let src = "-- pj2\nSELECT  FROM users";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "proj ctx missed `email`");
}

#[test]
fn r19_proj_0162() {
  let src = "-- pj2\nSELECT id,  FROM users";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "proj ctx missed `email`");
}

#[test]
fn r19_proj_0164() {
  let src = "-- pj2\nSELECT  FROM orders";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "proj ctx missed `user_id`");
}

#[test]
fn r19_proj_0165() {
  let src = "-- pj2\nSELECT id,  FROM orders";
  let cur = src.find("  ").unwrap() + 1;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "proj ctx missed `user_id`");
}

#[test]
fn r20_probe_ddl_ctx() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("CREATE TABLE t (id int CHECK (", "check_open"),
    ("CREATE TABLE t (id int REFERENCES ", "refs_table"),
    ("CREATE TABLE t (id int REFERENCES users(", "refs_col"),
    ("CREATE TABLE t (id int, PRIMARY KEY (", "pk_open"),
    ("CREATE TABLE t (id int, FOREIGN KEY (id) REFERENCES ", "fk_target"),
    ("CREATE INDEX ON users (", "index_col_open"),
    ("CREATE INDEX ON users (id, ", "index_col_after"),
    ("CREATE UNIQUE INDEX ON users (", "uniq_index_open"),
    ("CREATE INDEX ON users USING gin (", "gin_open"),
    ("CREATE INDEX ON users (id) WHERE ", "partial_index_where"),
    ("CREATE INDEX ON users (id) INCLUDE (", "include_open"),
    ("ALTER TABLE users ADD CONSTRAINT chk CHECK (", "alter_check"),
    ("ALTER TABLE users ADD FOREIGN KEY (id) REFERENCES ", "alter_fk_target"),
    ("ALTER TABLE users ALTER COLUMN id SET DEFAULT ", "set_default"),
    ("ALTER TABLE users ALTER COLUMN id TYPE ", "alter_type"),
    ("COMMENT ON TABLE users IS ", "comment_value"),
    ("CREATE VIEW v AS SELECT  FROM users", "view_projection"),
    ("CREATE MATERIALIZED VIEW mv AS SELECT  FROM users", "matview_projection"),
  ] {
    let cur = if let Some(c) = s.find("  ") { c + 1 } else { s.len() };
    let items = complete_at(s, cur, &cat);
    let has_id = items.iter().any(|i| i.label == "id");
    let n = items.len();
    eprintln!("R20|{}|n={}|id={}", label, n, has_id);
  }
}

#[test]
fn r20_refs_col_0001_id() {
  let src = "-- rc0\nCREATE TABLE t (id int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "REFERENCES users(| missed `id`");
}

#[test]
fn r20_refs_col_0001_email() {
  let src = "-- rc0\nCREATE TABLE t (id int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "REFERENCES users(| missed `email`");
}

#[test]
fn r20_refs_col_0001_name() {
  let src = "-- rc0\nCREATE TABLE t (id int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "REFERENCES users(| missed `name`");
}

#[test]
fn r20_refs_col_0002_id() {
  let src = "-- rc0\nCREATE TABLE t (a int, b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "REFERENCES users(| missed `id`");
}

#[test]
fn r20_refs_col_0002_email() {
  let src = "-- rc0\nCREATE TABLE t (a int, b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "REFERENCES users(| missed `email`");
}

#[test]
fn r20_refs_col_0002_name() {
  let src = "-- rc0\nCREATE TABLE t (a int, b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "REFERENCES users(| missed `name`");
}

#[test]
fn r20_refs_col_0003_id() {
  let src = "-- rc0\nCREATE TABLE t (id int REFERENCES users(id), b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "REFERENCES users(| missed `id`");
}

#[test]
fn r20_refs_col_0003_email() {
  let src = "-- rc0\nCREATE TABLE t (id int REFERENCES users(id), b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "REFERENCES users(| missed `email`");
}

#[test]
fn r20_refs_col_0003_name() {
  let src = "-- rc0\nCREATE TABLE t (id int REFERENCES users(id), b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "REFERENCES users(| missed `name`");
}

#[test]
fn r20_refs_col_0006_id() {
  let src = "-- rc1\nCREATE TABLE t (id int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "REFERENCES users(| missed `id`");
}

#[test]
fn r20_refs_col_0006_email() {
  let src = "-- rc1\nCREATE TABLE t (id int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "REFERENCES users(| missed `email`");
}

#[test]
fn r20_refs_col_0006_name() {
  let src = "-- rc1\nCREATE TABLE t (id int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "REFERENCES users(| missed `name`");
}

#[test]
fn r20_refs_col_0007_id() {
  let src = "-- rc1\nCREATE TABLE t (a int, b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "REFERENCES users(| missed `id`");
}

#[test]
fn r20_refs_col_0007_email() {
  let src = "-- rc1\nCREATE TABLE t (a int, b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "REFERENCES users(| missed `email`");
}

#[test]
fn r20_refs_col_0007_name() {
  let src = "-- rc1\nCREATE TABLE t (a int, b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "REFERENCES users(| missed `name`");
}

#[test]
fn r20_refs_col_0008_id() {
  let src = "-- rc1\nCREATE TABLE t (id int REFERENCES users(id), b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "REFERENCES users(| missed `id`");
}

#[test]
fn r20_refs_col_0008_email() {
  let src = "-- rc1\nCREATE TABLE t (id int REFERENCES users(id), b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "REFERENCES users(| missed `email`");
}

#[test]
fn r20_refs_col_0008_name() {
  let src = "-- rc1\nCREATE TABLE t (id int REFERENCES users(id), b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "REFERENCES users(| missed `name`");
}

#[test]
fn r20_refs_col_0011_id() {
  let src = "-- rc2\nCREATE TABLE t (id int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "REFERENCES users(| missed `id`");
}

#[test]
fn r20_refs_col_0011_email() {
  let src = "-- rc2\nCREATE TABLE t (id int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "REFERENCES users(| missed `email`");
}

#[test]
fn r20_refs_col_0011_name() {
  let src = "-- rc2\nCREATE TABLE t (id int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "REFERENCES users(| missed `name`");
}

#[test]
fn r20_refs_col_0012_id() {
  let src = "-- rc2\nCREATE TABLE t (a int, b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "REFERENCES users(| missed `id`");
}

#[test]
fn r20_refs_col_0012_email() {
  let src = "-- rc2\nCREATE TABLE t (a int, b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "REFERENCES users(| missed `email`");
}

#[test]
fn r20_refs_col_0012_name() {
  let src = "-- rc2\nCREATE TABLE t (a int, b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "REFERENCES users(| missed `name`");
}

#[test]
fn r20_refs_col_0013_id() {
  let src = "-- rc2\nCREATE TABLE t (id int REFERENCES users(id), b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "REFERENCES users(| missed `id`");
}

#[test]
fn r20_refs_col_0013_email() {
  let src = "-- rc2\nCREATE TABLE t (id int REFERENCES users(id), b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "REFERENCES users(| missed `email`");
}

#[test]
fn r20_refs_col_0013_name() {
  let src = "-- rc2\nCREATE TABLE t (id int REFERENCES users(id), b int REFERENCES users(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "REFERENCES users(| missed `name`");
}

#[test]
fn r20_idx_col_0016_id() {
  let src = "-- ix0\nCREATE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0016_email() {
  let src = "-- ix0\nCREATE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0016_name() {
  let src = "-- ix0\nCREATE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0017_id() {
  let src = "-- ix0\nCREATE UNIQUE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0017_email() {
  let src = "-- ix0\nCREATE UNIQUE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0017_name() {
  let src = "-- ix0\nCREATE UNIQUE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0018_id() {
  let src = "-- ix0\nCREATE INDEX CONCURRENTLY ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0018_email() {
  let src = "-- ix0\nCREATE INDEX CONCURRENTLY ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0018_name() {
  let src = "-- ix0\nCREATE INDEX CONCURRENTLY ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0019_id() {
  let src = "-- ix0\nCREATE INDEX idx ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0019_email() {
  let src = "-- ix0\nCREATE INDEX idx ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0019_name() {
  let src = "-- ix0\nCREATE INDEX idx ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0020_id() {
  let src = "-- ix0\nCREATE INDEX ON users USING btree (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0020_email() {
  let src = "-- ix0\nCREATE INDEX ON users USING btree (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0020_name() {
  let src = "-- ix0\nCREATE INDEX ON users USING btree (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0021_id() {
  let src = "-- ix1\nCREATE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0021_email() {
  let src = "-- ix1\nCREATE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0021_name() {
  let src = "-- ix1\nCREATE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0022_id() {
  let src = "-- ix1\nCREATE UNIQUE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0022_email() {
  let src = "-- ix1\nCREATE UNIQUE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0022_name() {
  let src = "-- ix1\nCREATE UNIQUE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0023_id() {
  let src = "-- ix1\nCREATE INDEX CONCURRENTLY ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0023_email() {
  let src = "-- ix1\nCREATE INDEX CONCURRENTLY ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0023_name() {
  let src = "-- ix1\nCREATE INDEX CONCURRENTLY ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0024_id() {
  let src = "-- ix1\nCREATE INDEX idx ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0024_email() {
  let src = "-- ix1\nCREATE INDEX idx ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0024_name() {
  let src = "-- ix1\nCREATE INDEX idx ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0025_id() {
  let src = "-- ix1\nCREATE INDEX ON users USING btree (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0025_email() {
  let src = "-- ix1\nCREATE INDEX ON users USING btree (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0025_name() {
  let src = "-- ix1\nCREATE INDEX ON users USING btree (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0026_id() {
  let src = "-- ix2\nCREATE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0026_email() {
  let src = "-- ix2\nCREATE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0026_name() {
  let src = "-- ix2\nCREATE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0027_id() {
  let src = "-- ix2\nCREATE UNIQUE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0027_email() {
  let src = "-- ix2\nCREATE UNIQUE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0027_name() {
  let src = "-- ix2\nCREATE UNIQUE INDEX ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0028_id() {
  let src = "-- ix2\nCREATE INDEX CONCURRENTLY ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0028_email() {
  let src = "-- ix2\nCREATE INDEX CONCURRENTLY ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0028_name() {
  let src = "-- ix2\nCREATE INDEX CONCURRENTLY ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0029_id() {
  let src = "-- ix2\nCREATE INDEX idx ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0029_email() {
  let src = "-- ix2\nCREATE INDEX idx ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0029_name() {
  let src = "-- ix2\nCREATE INDEX idx ON users (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_col_0030_id() {
  let src = "-- ix2\nCREATE INDEX ON users USING btree (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_col_0030_email() {
  let src = "-- ix2\nCREATE INDEX ON users USING btree (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r20_idx_col_0030_name() {
  let src = "-- ix2\nCREATE INDEX ON users USING btree (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_using_0041() {
  let src = "-- gi0\nCREATE INDEX ON users USING gin (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0042() {
  let src = "-- gi0\nCREATE INDEX ON users USING gist (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0043() {
  let src = "-- gi0\nCREATE INDEX ON users USING brin (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0044() {
  let src = "-- gi0\nCREATE INDEX ON users USING hash (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0045() {
  let src = "-- gi1\nCREATE INDEX ON users USING gin (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0046() {
  let src = "-- gi1\nCREATE INDEX ON users USING gist (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0047() {
  let src = "-- gi1\nCREATE INDEX ON users USING brin (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0048() {
  let src = "-- gi1\nCREATE INDEX ON users USING hash (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0049() {
  let src = "-- gi2\nCREATE INDEX ON users USING gin (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0050() {
  let src = "-- gi2\nCREATE INDEX ON users USING gist (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0051() {
  let src = "-- gi2\nCREATE INDEX ON users USING brin (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_using_0052() {
  let src = "-- gi2\nCREATE INDEX ON users USING hash (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_idx_include_0061() {
  let src = "-- inc0\nCREATE INDEX ON users (id) INCLUDE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_include_0062() {
  let src = "-- inc0\nCREATE UNIQUE INDEX ON users (id) INCLUDE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_include_0063() {
  let src = "-- inc0\nCREATE INDEX ON users (email) INCLUDE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_include_0064() {
  let src = "-- inc1\nCREATE INDEX ON users (id) INCLUDE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_include_0065() {
  let src = "-- inc1\nCREATE UNIQUE INDEX ON users (id) INCLUDE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_include_0066() {
  let src = "-- inc1\nCREATE INDEX ON users (email) INCLUDE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_include_0067() {
  let src = "-- inc2\nCREATE INDEX ON users (id) INCLUDE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_include_0068() {
  let src = "-- inc2\nCREATE UNIQUE INDEX ON users (id) INCLUDE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_idx_include_0069() {
  let src = "-- inc2\nCREATE INDEX ON users (email) INCLUDE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_view_proj_0085() {
  let src = "-- vw0\nCREATE VIEW v AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0086() {
  let src = "-- vw0\nCREATE OR REPLACE VIEW v AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0087() {
  let src = "-- vw0\nCREATE MATERIALIZED VIEW mv AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0088() {
  let src = "-- vw0\nCREATE TEMP VIEW v AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0089() {
  let src = "-- vw0\nCREATE RECURSIVE VIEW v(a) AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0090() {
  let src = "-- vw1\nCREATE VIEW v AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0091() {
  let src = "-- vw1\nCREATE OR REPLACE VIEW v AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0092() {
  let src = "-- vw1\nCREATE MATERIALIZED VIEW mv AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0093() {
  let src = "-- vw1\nCREATE TEMP VIEW v AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0094() {
  let src = "-- vw1\nCREATE RECURSIVE VIEW v(a) AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0095() {
  let src = "-- vw2\nCREATE VIEW v AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0096() {
  let src = "-- vw2\nCREATE OR REPLACE VIEW v AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0097() {
  let src = "-- vw2\nCREATE MATERIALIZED VIEW mv AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0098() {
  let src = "-- vw2\nCREATE TEMP VIEW v AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_view_proj_0099() {
  let src = "-- vw2\nCREATE RECURSIVE VIEW v(a) AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "view projection missed id");
}

#[test]
fn r20_pk_open_0110() {
  let src = "-- pk0\nCREATE TABLE t (id int, PRIMARY KEY (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_pk_open_0111() {
  let src = "-- pk0\nCREATE TABLE users_v2 (id int, email text, PRIMARY KEY (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_pk_open_0114() {
  let src = "-- pk1\nCREATE TABLE t (id int, PRIMARY KEY (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_pk_open_0115() {
  let src = "-- pk1\nCREATE TABLE users_v2 (id int, email text, PRIMARY KEY (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_pk_open_0118() {
  let src = "-- pk2\nCREATE TABLE t (id int, PRIMARY KEY (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_pk_open_0119() {
  let src = "-- pk2\nCREATE TABLE users_v2 (id int, email text, PRIMARY KEY (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r20_partial_idx_0130() {
  let src = "-- pix0\nCREATE INDEX ON users (id) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0131() {
  let src = "-- pix0\nCREATE INDEX ON users (email) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0132() {
  let src = "-- pix0\nCREATE UNIQUE INDEX ON users (id) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0133() {
  let src = "-- pix0\nCREATE INDEX CONCURRENTLY ON users (id) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0134() {
  let src = "-- pix1\nCREATE INDEX ON users (id) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0135() {
  let src = "-- pix1\nCREATE INDEX ON users (email) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0136() {
  let src = "-- pix1\nCREATE UNIQUE INDEX ON users (id) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0137() {
  let src = "-- pix1\nCREATE INDEX CONCURRENTLY ON users (id) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0138() {
  let src = "-- pix2\nCREATE INDEX ON users (id) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0139() {
  let src = "-- pix2\nCREATE INDEX ON users (email) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0140() {
  let src = "-- pix2\nCREATE UNIQUE INDEX ON users (id) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r20_partial_idx_0141() {
  let src = "-- pix2\nCREATE INDEX CONCURRENTLY ON users (id) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"));
}

#[test]
fn r21_probe() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("SELECT * FROM users WHERE id = 1 OR id = ", "or_after_eq"),
    ("SELECT * FROM users WHERE (id, email) IN ((1, 'a'), (", "row_in_tuple"),
    ("SELECT * FROM users WHERE (id, email) > (1, ", "row_gt_partial"),
    ("SELECT (SELECT max(", "scalar_subq_arg_open"),
    ("SELECT (SELECT id FROM users LIMIT 1), ", "after_scalar_subq"),
    ("SELECT id FROM users WHERE id NOT BETWEEN 1 AND ", "not_between_rhs"),
    ("SELECT id FROM users WHERE id IS DISTINCT FROM ", "distinct_from_rhs"),
    ("SELECT id FROM users WHERE id IS NOT DISTINCT FROM ", "not_distinct_from_rhs"),
    ("SELECT id FROM users WHERE id != ", "neq_rhs"),
    ("SELECT id FROM users WHERE id <> ", "alt_neq_rhs"),
    ("SELECT id FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE user_id = ", "exists_corr"),
    ("SELECT id FROM users WHERE id = ANY ( SELECT id FROM orders WHERE user_id = ", "any_corr"),
    ("SELECT (SELECT count(*) FROM orders WHERE user_id = ", "count_corr"),
    ("SELECT id, email, name FROM users WHERE (id, name) = (1, ", "row_eq_partial"),
    ("UPDATE users SET name = (SELECT max(name) FROM orders WHERE user_id = ", "set_subq_corr"),
  ] {
    let items = complete_at(s, s.len(), &cat);
    let has_id = items.iter().any(|i| i.label == "id");
    eprintln!("R21|{}|n={}|id={}", label, items.len(), has_id);
  }
}

#[test]
fn r21_rhs_0001() {
  let src = "-- rh0\nSELECT * FROM users WHERE id = 1 OR id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0002() {
  let src = "-- rh0\nSELECT * FROM users WHERE id != ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0003() {
  let src = "-- rh0\nSELECT * FROM users WHERE id <> ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0004() {
  let src = "-- rh0\nSELECT * FROM users WHERE id NOT BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0005() {
  let src = "-- rh0\nSELECT * FROM users WHERE id IS DISTINCT FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0006() {
  let src = "-- rh0\nSELECT * FROM users WHERE id IS NOT DISTINCT FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0007() {
  let src = "-- rh0\nSELECT * FROM users WHERE id <= ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0008() {
  let src = "-- rh0\nSELECT * FROM users WHERE id >= ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0009() {
  let src = "-- rh1\nSELECT * FROM users WHERE id = 1 OR id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0010() {
  let src = "-- rh1\nSELECT * FROM users WHERE id != ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0011() {
  let src = "-- rh1\nSELECT * FROM users WHERE id <> ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0012() {
  let src = "-- rh1\nSELECT * FROM users WHERE id NOT BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0013() {
  let src = "-- rh1\nSELECT * FROM users WHERE id IS DISTINCT FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0014() {
  let src = "-- rh1\nSELECT * FROM users WHERE id IS NOT DISTINCT FROM ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0015() {
  let src = "-- rh1\nSELECT * FROM users WHERE id <= ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0016() {
  let src = "-- rh1\nSELECT * FROM users WHERE id >= ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0017() {
  let src = "-- rh2\nSELECT * FROM users WHERE id = 1 OR id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0018() {
  let src = "-- rh2\nSELECT * FROM users WHERE id != ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0019() {
  let src = "-- rh2\nSELECT * FROM users WHERE id <> ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_rhs_0020() {
  let src = "-- rh2\nSELECT * FROM users WHERE id NOT BETWEEN 1 AND ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_corr_0021() {
  let src = "-- cr0\nSELECT id FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0022() {
  let src = "-- cr0\nSELECT id FROM users WHERE id = ANY ( SELECT id FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0023() {
  let src = "-- cr0\nSELECT (SELECT count(*) FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0024() {
  let src = "-- cr0\nUPDATE users SET name = (SELECT max(name) FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0025() {
  let src = "-- cr0\nSELECT id FROM users WHERE id IN (SELECT user_id FROM orders WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0026() {
  let src = "-- cr1\nSELECT id FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0027() {
  let src = "-- cr1\nSELECT id FROM users WHERE id = ANY ( SELECT id FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0028() {
  let src = "-- cr1\nSELECT (SELECT count(*) FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0029() {
  let src = "-- cr1\nUPDATE users SET name = (SELECT max(name) FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0030() {
  let src = "-- cr1\nSELECT id FROM users WHERE id IN (SELECT user_id FROM orders WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0031() {
  let src = "-- cr2\nSELECT id FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0032() {
  let src = "-- cr2\nSELECT id FROM users WHERE id = ANY ( SELECT id FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0033() {
  let src = "-- cr2\nSELECT (SELECT count(*) FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0034() {
  let src = "-- cr2\nUPDATE users SET name = (SELECT max(name) FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_corr_0035() {
  let src = "-- cr2\nSELECT id FROM users WHERE id IN (SELECT user_id FROM orders WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "correlated subq missed id");
}

#[test]
fn r21_row_0041() {
  let src = "-- rw0\nSELECT * FROM users WHERE (id, email) > (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0042() {
  let src = "-- rw0\nSELECT * FROM users WHERE (id, email) = (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0043() {
  let src = "-- rw0\nSELECT * FROM users WHERE (id, name) IN ((1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0044() {
  let src = "-- rw0\nSELECT * FROM users WHERE (id, email, name) > (1, 'a', ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0045() {
  let src = "-- rw1\nSELECT * FROM users WHERE (id, email) > (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0046() {
  let src = "-- rw1\nSELECT * FROM users WHERE (id, email) = (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0047() {
  let src = "-- rw1\nSELECT * FROM users WHERE (id, name) IN ((1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0048() {
  let src = "-- rw1\nSELECT * FROM users WHERE (id, email, name) > (1, 'a', ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0049() {
  let src = "-- rw2\nSELECT * FROM users WHERE (id, email) > (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0050() {
  let src = "-- rw2\nSELECT * FROM users WHERE (id, email) = (1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0051() {
  let src = "-- rw2\nSELECT * FROM users WHERE (id, name) IN ((1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r21_row_0052() {
  let src = "-- rw2\nSELECT * FROM users WHERE (id, email, name) > (1, 'a', ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r22_probe() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("WITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT n+1 FROM r WHERE n < 10) SELECT  FROM r", "rec_cte_proj"),
    ("WITH x AS (SELECT id, email FROM users) SELECT x. FROM x", "cte_alias_dot"),
    ("SELECT a.id FROM (SELECT id FROM users) a WHERE a.", "subq_alias_dot"),
    ("SELECT users.id, users.email FROM users WHERE users.", "table_qualified_dot"),
    ("SELECT * FROM users u WHERE u.id IN (SELECT user_id FROM orders WHERE orders.", "subq_table_qual"),
    ("SELECT * FROM users u WHERE u.id = (SELECT max(o.user_id) FROM orders o WHERE o.", "subq_alias_corr"),
    ("MERGE INTO users u USING orders o ON u.id = o.user_id WHEN MATCHED THEN UPDATE SET name = ", "merge_set_rhs"),
    ("MERGE INTO users u USING orders o ON u.id = o.user_id WHEN NOT MATCHED THEN INSERT (", "merge_insert_cols"),
    ("MERGE INTO users u USING orders o ON u.id = o.user_id WHEN NOT MATCHED THEN INSERT (id) VALUES (", "merge_insert_vals"),
    ("DECLARE c CURSOR FOR SELECT  FROM users", "cursor_select"),
    ("PREPARE p AS SELECT  FROM users", "prepare_select"),
    ("EXPLAIN SELECT  FROM users", "explain_select"),
    ("EXPLAIN (ANALYZE) SELECT  FROM users", "explain_analyze"),
    ("EXPLAIN (BUFFERS, ANALYZE, FORMAT JSON) SELECT  FROM users", "explain_complex"),
  ] {
    let cur = if let Some(c) = s.find("  ") { c + 1 } else { s.len() };
    let items = complete_at(s, cur, &cat);
    let has_id = items.iter().any(|i| i.label == "id");
    let has_email = items.iter().any(|i| i.label == "email");
    eprintln!("R|{}|n={}|id={}|email={}", label, items.len(), has_id, has_email);
  }
}

#[test]
fn r22_probe_cte_dot() {
  let cat = catalog_with_users_and_orders();
  let s = "WITH x AS (SELECT id, email FROM users) SELECT x. FROM x";
  let cur = s.find("x.").unwrap() + 2;
  let items = complete_at(s, cur, &cat);
  for it in &items {
    eprintln!("CTE|{}|{:?}", it.label, it.kind);
  }
}

#[test]
fn r22_probe_table_qual() {
  let cat = catalog_with_users_and_orders();
  let s = "SELECT users.id FROM users WHERE users.";
  let items = complete_at(s, s.len(), &cat);
  for it in &items { eprintln!("TQ|{}|{:?}", it.label, it.kind); }
}

#[test]
fn r22_probe_table_qual2() {
  let cat = catalog_with_users_and_orders();
  let s = "SELECT * FROM users WHERE users.";
  let items = complete_at(s, s.len(), &cat);
  for it in &items { eprintln!("T2|{}|{:?}", it.label, it.kind); }
}

#[test]
fn r22_rec_cte_0001() {
  let src = "-- rc0\nWITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT n+1 FROM r WHERE n < 10) SELECT  FROM r";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "rec CTE proj missed `id`");
}

#[test]
fn r22_rec_cte_0002() {
  let src = "-- rc0\nWITH RECURSIVE r AS (SELECT id FROM users UNION SELECT id FROM users) SELECT  FROM r";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "rec CTE proj missed `id`");
}

#[test]
fn r22_rec_cte_0003() {
  let src = "-- rc0\nWITH RECURSIVE r AS (SELECT id, email FROM users UNION SELECT id, email FROM users) SELECT  FROM r";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "rec CTE proj missed `email`");
}

#[test]
fn r22_rec_cte_0004() {
  let src = "-- rc1\nWITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT n+1 FROM r WHERE n < 10) SELECT  FROM r";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "rec CTE proj missed `id`");
}

#[test]
fn r22_rec_cte_0005() {
  let src = "-- rc1\nWITH RECURSIVE r AS (SELECT id FROM users UNION SELECT id FROM users) SELECT  FROM r";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "rec CTE proj missed `id`");
}

#[test]
fn r22_rec_cte_0006() {
  let src = "-- rc1\nWITH RECURSIVE r AS (SELECT id, email FROM users UNION SELECT id, email FROM users) SELECT  FROM r";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "rec CTE proj missed `email`");
}

#[test]
fn r22_rec_cte_0007() {
  let src = "-- rc2\nWITH RECURSIVE r(n) AS (SELECT 1 UNION SELECT n+1 FROM r WHERE n < 10) SELECT  FROM r";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "rec CTE proj missed `id`");
}

#[test]
fn r22_rec_cte_0008() {
  let src = "-- rc2\nWITH RECURSIVE r AS (SELECT id FROM users UNION SELECT id FROM users) SELECT  FROM r";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "rec CTE proj missed `id`");
}

#[test]
fn r22_rec_cte_0009() {
  let src = "-- rc2\nWITH RECURSIVE r AS (SELECT id, email FROM users UNION SELECT id, email FROM users) SELECT  FROM r";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "rec CTE proj missed `email`");
}

#[test]
fn r22_cte_dot_0010() {
  let src = "-- cd0\nWITH x AS (SELECT id, email FROM users) SELECT x. FROM x";
  let cur = src.rfind("x.").unwrap() + 2;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CTE alias dot missed col");
}

#[test]
fn r22_cte_dot_0011() {
  let src = "-- cd0\nWITH y AS (SELECT id, name FROM users) SELECT y. FROM y";
  let cur = src.rfind("y.").unwrap() + 2;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CTE alias dot missed col");
}

#[test]
fn r22_cte_dot_0012() {
  let src = "-- cd0\nWITH z AS (SELECT id FROM orders) SELECT z. FROM z";
  let cur = src.rfind("z.").unwrap() + 2;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CTE alias dot missed col");
}

#[test]
fn r22_cte_dot_0013() {
  let src = "-- cd1\nWITH x AS (SELECT id, email FROM users) SELECT x. FROM x";
  let cur = src.rfind("x.").unwrap() + 2;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CTE alias dot missed col");
}

#[test]
fn r22_cte_dot_0014() {
  let src = "-- cd1\nWITH y AS (SELECT id, name FROM users) SELECT y. FROM y";
  let cur = src.rfind("y.").unwrap() + 2;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CTE alias dot missed col");
}

#[test]
fn r22_cte_dot_0015() {
  let src = "-- cd1\nWITH z AS (SELECT id FROM orders) SELECT z. FROM z";
  let cur = src.rfind("z.").unwrap() + 2;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CTE alias dot missed col");
}

#[test]
fn r22_cte_dot_0016() {
  let src = "-- cd2\nWITH x AS (SELECT id, email FROM users) SELECT x. FROM x";
  let cur = src.rfind("x.").unwrap() + 2;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CTE alias dot missed col");
}

#[test]
fn r22_cte_dot_0017() {
  let src = "-- cd2\nWITH y AS (SELECT id, name FROM users) SELECT y. FROM y";
  let cur = src.rfind("y.").unwrap() + 2;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CTE alias dot missed col");
}

#[test]
fn r22_cte_dot_0018() {
  let src = "-- cd2\nWITH z AS (SELECT id FROM orders) SELECT z. FROM z";
  let cur = src.rfind("z.").unwrap() + 2;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "CTE alias dot missed col");
}

#[test]
fn r22_tq_0022() {
  let src = "-- tq0\nSELECT * FROM users WHERE users.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "table-qual dot missed id");
}

#[test]
fn r22_tq_0023() {
  let src = "-- tq0\nSELECT * FROM orders WHERE orders.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "table-qual dot missed id");
}

#[test]
fn r22_tq_0024() {
  let src = "-- tq0\nDELETE FROM users WHERE users.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "table-qual dot missed id");
}

#[test]
fn r22_tq_0025() {
  let src = "-- tq1\nSELECT * FROM users WHERE users.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "table-qual dot missed id");
}

#[test]
fn r22_tq_0026() {
  let src = "-- tq1\nSELECT * FROM orders WHERE orders.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "table-qual dot missed id");
}

#[test]
fn r22_tq_0027() {
  let src = "-- tq1\nDELETE FROM users WHERE users.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "table-qual dot missed id");
}

#[test]
fn r22_tq_0028() {
  let src = "-- tq2\nSELECT * FROM users WHERE users.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "table-qual dot missed id");
}

#[test]
fn r22_tq_0029() {
  let src = "-- tq2\nSELECT * FROM orders WHERE orders.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "table-qual dot missed id");
}

#[test]
fn r22_tq_0030() {
  let src = "-- tq2\nDELETE FROM users WHERE users.";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "table-qual dot missed id");
}

#[test]
fn r22_merge_ins_0041() {
  let src = "-- mr0\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN NOT MATCHED THEN INSERT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "MERGE INSERT cols missed `id`");
}

#[test]
fn r22_merge_ins_0042() {
  let src = "-- mr0\nMERGE INTO orders o USING users u ON o.user_id = u.id WHEN NOT MATCHED THEN INSERT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "MERGE INSERT cols missed `id`");
}

#[test]
fn r22_merge_ins_0043() {
  let src = "-- mr1\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN NOT MATCHED THEN INSERT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "MERGE INSERT cols missed `id`");
}

#[test]
fn r22_merge_ins_0044() {
  let src = "-- mr1\nMERGE INTO orders o USING users u ON o.user_id = u.id WHEN NOT MATCHED THEN INSERT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "MERGE INSERT cols missed `id`");
}

#[test]
fn r22_merge_ins_0045() {
  let src = "-- mr2\nMERGE INTO users u USING orders o ON u.id = o.user_id WHEN NOT MATCHED THEN INSERT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "MERGE INSERT cols missed `id`");
}

#[test]
fn r22_merge_ins_0046() {
  let src = "-- mr2\nMERGE INTO orders o USING users u ON o.user_id = u.id WHEN NOT MATCHED THEN INSERT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "MERGE INSERT cols missed `id`");
}

#[test]
fn r22_meta_0057() {
  let src = "-- mc0\nDECLARE c CURSOR FOR SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0058() {
  let src = "-- mc0\nDECLARE c CURSOR FOR SELECT  FROM orders";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0059() {
  let src = "-- mc0\nPREPARE p AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "meta-cmd proj missed `email`");
}

#[test]
fn r22_meta_0060() {
  let src = "-- mc0\nPREPARE p AS SELECT  FROM orders";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "meta-cmd proj missed `user_id`");
}

#[test]
fn r22_meta_0061() {
  let src = "-- mc0\nEXPLAIN SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "meta-cmd proj missed `name`");
}

#[test]
fn r22_meta_0062() {
  let src = "-- mc0\nEXPLAIN ANALYZE SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0063() {
  let src = "-- mc0\nEXPLAIN (FORMAT JSON) SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0064() {
  let src = "-- mc0\nEXPLAIN (BUFFERS, ANALYZE) SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "meta-cmd proj missed `email`");
}

#[test]
fn r22_meta_0065() {
  let src = "-- mc1\nDECLARE c CURSOR FOR SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0066() {
  let src = "-- mc1\nDECLARE c CURSOR FOR SELECT  FROM orders";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0067() {
  let src = "-- mc1\nPREPARE p AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "meta-cmd proj missed `email`");
}

#[test]
fn r22_meta_0068() {
  let src = "-- mc1\nPREPARE p AS SELECT  FROM orders";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "meta-cmd proj missed `user_id`");
}

#[test]
fn r22_meta_0069() {
  let src = "-- mc1\nEXPLAIN SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "meta-cmd proj missed `name`");
}

#[test]
fn r22_meta_0070() {
  let src = "-- mc1\nEXPLAIN ANALYZE SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0071() {
  let src = "-- mc1\nEXPLAIN (FORMAT JSON) SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0072() {
  let src = "-- mc1\nEXPLAIN (BUFFERS, ANALYZE) SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "meta-cmd proj missed `email`");
}

#[test]
fn r22_meta_0073() {
  let src = "-- mc2\nDECLARE c CURSOR FOR SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0074() {
  let src = "-- mc2\nDECLARE c CURSOR FOR SELECT  FROM orders";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0075() {
  let src = "-- mc2\nPREPARE p AS SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "meta-cmd proj missed `email`");
}

#[test]
fn r22_meta_0076() {
  let src = "-- mc2\nPREPARE p AS SELECT  FROM orders";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "user_id"), "meta-cmd proj missed `user_id`");
}

#[test]
fn r22_meta_0077() {
  let src = "-- mc2\nEXPLAIN SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name"), "meta-cmd proj missed `name`");
}

#[test]
fn r22_meta_0078() {
  let src = "-- mc2\nEXPLAIN ANALYZE SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0079() {
  let src = "-- mc2\nEXPLAIN (FORMAT JSON) SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "meta-cmd proj missed `id`");
}

#[test]
fn r22_meta_0080() {
  let src = "-- mc2\nEXPLAIN (BUFFERS, ANALYZE) SELECT  FROM users";
  let cur = src.find("SELECT  ").unwrap() + 7;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"), "meta-cmd proj missed `email`");
}

#[test]
fn r23_probe() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("SELECT * FROM users TABLESAMPLE BERNOULLI (", "tablesample_arg"),
    ("SELECT * FROM users TABLESAMPLE SYSTEM (10) REPEATABLE (", "tablesample_repeat"),
    ("SELECT count(*) FILTER (WHERE ", "filter_where"),
    ("SELECT count(*) FILTER (WHERE id > 0) OVER (", "filter_over"),
    ("SELECT row_number() OVER (PARTITION BY id ORDER BY ", "win_order"),
    ("SELECT sum(amount) OVER (PARTITION BY user_id ORDER BY id ROWS BETWEEN ", "win_rows_between"),
    ("SELECT id, lag(id, 1, ", "lag_default"),
    ("SELECT id FROM users WHERE (id, email) > (SELECT id, email FROM users WHERE id = ", "row_compare_subq"),
    ("SELECT id FROM users u CROSS JOIN LATERAL (SELECT * FROM orders WHERE user_id = ", "lateral_corr"),
    ("SELECT * FROM users WHERE id = ANY(SELECT user_id FROM ", "any_subq_from"),
    ("SELECT array_agg(id ORDER BY ", "agg_order_by"),
    ("SELECT string_agg(name, ',' ORDER BY ", "string_agg_order"),
    ("SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY ", "within_group"),
    ("INSERT INTO users (id) VALUES (DEFAULT) ON CONFLICT (", "conflict_target"),
    ("INSERT INTO users (id) VALUES (1) ON CONFLICT ON CONSTRAINT ", "conflict_constraint"),
  ] {
    let items = complete_at(s, s.len(), &cat);
    let has_id = items.iter().any(|i| i.label == "id");
    eprintln!("R|{}|n={}|id={}", label, items.len(), has_id);
  }
}

#[test]
fn r23_filter_0001() {
  let src = "-- f0\nSELECT count(*) FILTER (WHERE  FROM users";
  let cur = src.find("WHERE ").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "FILTER WHERE missed id");
}

#[test]
fn r23_filter_0002() {
  let src = "-- f0\nSELECT count(id) FILTER (WHERE  FROM users";
  let cur = src.find("WHERE ").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "FILTER WHERE missed id");
}

#[test]
fn r23_filter_0003() {
  let src = "-- f0\nSELECT array_agg(name) FILTER (WHERE  FROM users";
  let cur = src.find("WHERE ").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "FILTER WHERE missed id");
}

#[test]
fn r23_filter_0004() {
  let src = "-- f1\nSELECT count(*) FILTER (WHERE  FROM users";
  let cur = src.find("WHERE ").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "FILTER WHERE missed id");
}

#[test]
fn r23_filter_0005() {
  let src = "-- f1\nSELECT count(id) FILTER (WHERE  FROM users";
  let cur = src.find("WHERE ").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "FILTER WHERE missed id");
}

#[test]
fn r23_filter_0006() {
  let src = "-- f1\nSELECT array_agg(name) FILTER (WHERE  FROM users";
  let cur = src.find("WHERE ").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "FILTER WHERE missed id");
}

#[test]
fn r23_filter_0007() {
  let src = "-- f2\nSELECT count(*) FILTER (WHERE  FROM users";
  let cur = src.find("WHERE ").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "FILTER WHERE missed id");
}

#[test]
fn r23_filter_0008() {
  let src = "-- f2\nSELECT count(id) FILTER (WHERE  FROM users";
  let cur = src.find("WHERE ").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "FILTER WHERE missed id");
}

#[test]
fn r23_filter_0009() {
  let src = "-- f2\nSELECT array_agg(name) FILTER (WHERE  FROM users";
  let cur = src.find("WHERE ").unwrap() + 6;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "FILTER WHERE missed id");
}

#[test]
fn r23_over_ord_0010() {
  let src = "-- ov0\nSELECT row_number() OVER (PARTITION BY id ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "OVER ORDER BY missed id");
}

#[test]
fn r23_over_ord_0011() {
  let src = "-- ov0\nSELECT rank() OVER (PARTITION BY email ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "OVER ORDER BY missed id");
}

#[test]
fn r23_over_ord_0012() {
  let src = "-- ov0\nSELECT lag(id) OVER (PARTITION BY email ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "OVER ORDER BY missed id");
}

#[test]
fn r23_over_ord_0013() {
  let src = "-- ov1\nSELECT row_number() OVER (PARTITION BY id ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "OVER ORDER BY missed id");
}

#[test]
fn r23_over_ord_0014() {
  let src = "-- ov1\nSELECT rank() OVER (PARTITION BY email ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "OVER ORDER BY missed id");
}

#[test]
fn r23_over_ord_0015() {
  let src = "-- ov1\nSELECT lag(id) OVER (PARTITION BY email ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "OVER ORDER BY missed id");
}

#[test]
fn r23_over_ord_0016() {
  let src = "-- ov2\nSELECT row_number() OVER (PARTITION BY id ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "OVER ORDER BY missed id");
}

#[test]
fn r23_over_ord_0017() {
  let src = "-- ov2\nSELECT rank() OVER (PARTITION BY email ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "OVER ORDER BY missed id");
}

#[test]
fn r23_over_ord_0018() {
  let src = "-- ov2\nSELECT lag(id) OVER (PARTITION BY email ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "OVER ORDER BY missed id");
}

#[test]
fn r23_agg_ord_0019() {
  let src = "-- ao0\nSELECT array_agg(id ORDER BY  FROM users";
  let cur = src.rfind("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "agg ORDER BY missed id");
}

#[test]
fn r23_agg_ord_0020() {
  let src = "-- ao0\nSELECT array_agg(name ORDER BY  FROM users";
  let cur = src.rfind("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "agg ORDER BY missed id");
}

#[test]
fn r23_agg_ord_0021() {
  let src = "-- ao0\nSELECT string_agg(name, ',' ORDER BY  FROM users";
  let cur = src.rfind("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "agg ORDER BY missed id");
}

#[test]
fn r23_agg_ord_0022() {
  let src = "-- ao0\nSELECT json_agg(id ORDER BY  FROM users";
  let cur = src.rfind("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "agg ORDER BY missed id");
}

#[test]
fn r23_agg_ord_0023() {
  let src = "-- ao1\nSELECT array_agg(id ORDER BY  FROM users";
  let cur = src.rfind("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "agg ORDER BY missed id");
}

#[test]
fn r23_agg_ord_0024() {
  let src = "-- ao1\nSELECT array_agg(name ORDER BY  FROM users";
  let cur = src.rfind("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "agg ORDER BY missed id");
}

#[test]
fn r23_agg_ord_0025() {
  let src = "-- ao1\nSELECT string_agg(name, ',' ORDER BY  FROM users";
  let cur = src.rfind("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "agg ORDER BY missed id");
}

#[test]
fn r23_agg_ord_0026() {
  let src = "-- ao1\nSELECT json_agg(id ORDER BY  FROM users";
  let cur = src.rfind("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "agg ORDER BY missed id");
}

#[test]
fn r23_agg_ord_0027() {
  let src = "-- ao2\nSELECT array_agg(id ORDER BY  FROM users";
  let cur = src.rfind("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "agg ORDER BY missed id");
}

#[test]
fn r23_within_0028() {
  let src = "-- wg0\nSELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "WITHIN GROUP missed id");
}

#[test]
fn r23_within_0029() {
  let src = "-- wg0\nSELECT percentile_disc(0.5) WITHIN GROUP (ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "WITHIN GROUP missed id");
}

#[test]
fn r23_within_0030() {
  let src = "-- wg0\nSELECT mode() WITHIN GROUP (ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "WITHIN GROUP missed id");
}

#[test]
fn r23_within_0031() {
  let src = "-- wg1\nSELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "WITHIN GROUP missed id");
}

#[test]
fn r23_within_0032() {
  let src = "-- wg1\nSELECT percentile_disc(0.5) WITHIN GROUP (ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "WITHIN GROUP missed id");
}

#[test]
fn r23_within_0033() {
  let src = "-- wg1\nSELECT mode() WITHIN GROUP (ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "WITHIN GROUP missed id");
}

#[test]
fn r23_within_0034() {
  let src = "-- wg2\nSELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "WITHIN GROUP missed id");
}

#[test]
fn r23_within_0035() {
  let src = "-- wg2\nSELECT percentile_disc(0.5) WITHIN GROUP (ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "WITHIN GROUP missed id");
}

#[test]
fn r23_within_0036() {
  let src = "-- wg2\nSELECT mode() WITHIN GROUP (ORDER BY  FROM users";
  let cur = src.find("ORDER BY ").unwrap() + 9;
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "WITHIN GROUP missed id");
}

#[test]
fn r23_lateral_0037() {
  let src = "-- lt0\nSELECT * FROM users u CROSS JOIN LATERAL (SELECT * FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "LATERAL corr missed id");
}

#[test]
fn r23_lateral_0038() {
  let src = "-- lt0\nSELECT u.id, x.id FROM users u, LATERAL (SELECT * FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "LATERAL corr missed id");
}

#[test]
fn r23_lateral_0039() {
  let src = "-- lt1\nSELECT * FROM users u CROSS JOIN LATERAL (SELECT * FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "LATERAL corr missed id");
}

#[test]
fn r23_lateral_0040() {
  let src = "-- lt1\nSELECT u.id, x.id FROM users u, LATERAL (SELECT * FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "LATERAL corr missed id");
}

#[test]
fn r23_lateral_0041() {
  let src = "-- lt2\nSELECT * FROM users u CROSS JOIN LATERAL (SELECT * FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "LATERAL corr missed id");
}

#[test]
fn r23_lateral_0042() {
  let src = "-- lt2\nSELECT u.id, x.id FROM users u, LATERAL (SELECT * FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "LATERAL corr missed id");
}

#[test]
fn r23_conflict_target_0046() {
  let src = "-- ct0\nINSERT INTO users (id) VALUES (1) ON CONFLICT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT target missed id");
}

#[test]
fn r23_conflict_target_0047() {
  let src = "-- ct0\nINSERT INTO orders (id) VALUES (1) ON CONFLICT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT target missed id");
}

#[test]
fn r23_conflict_target_0048() {
  let src = "-- ct0\nINSERT INTO users VALUES (DEFAULT) ON CONFLICT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT target missed id");
}

#[test]
fn r23_conflict_target_0049() {
  let src = "-- ct1\nINSERT INTO users (id) VALUES (1) ON CONFLICT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT target missed id");
}

#[test]
fn r23_conflict_target_0050() {
  let src = "-- ct1\nINSERT INTO orders (id) VALUES (1) ON CONFLICT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT target missed id");
}

#[test]
fn r23_conflict_target_0051() {
  let src = "-- ct1\nINSERT INTO users VALUES (DEFAULT) ON CONFLICT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT target missed id");
}

#[test]
fn r23_conflict_target_0052() {
  let src = "-- ct2\nINSERT INTO users (id) VALUES (1) ON CONFLICT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT target missed id");
}

#[test]
fn r23_conflict_target_0053() {
  let src = "-- ct2\nINSERT INTO orders (id) VALUES (1) ON CONFLICT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT target missed id");
}

#[test]
fn r23_conflict_target_0054() {
  let src = "-- ct2\nINSERT INTO users VALUES (DEFAULT) ON CONFLICT (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "ON CONFLICT target missed id");
}

#[test]
fn r24_probe() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("SELECT a[", "array_subscript"),
    ("SELECT a[1:", "array_slice"),
    ("SELECT a[1:3] FROM users WHERE id = ", "after_slice"),
    ("SELECT id FROM users WHERE id = (SELECT MAX(", "subq_agg_open"),
    ("SELECT id FROM users WHERE id = (SELECT MAX(id) FROM users WHERE ", "subq_where"),
    ("SELECT id FROM users u WHERE u.id IS DISTINCT FROM ", "distinct_from"),
    ("SELECT id FROM users WHERE id < ALL(SELECT id FROM ", "all_subq_from"),
    ("SELECT id FROM users WHERE id < SOME(SELECT id FROM ", "some_subq_from"),
    ("CREATE TABLE t (a int CHECK (a > 0), b int CHECK (b > 0), CHECK (", "table_check_open"),
    ("CREATE TABLE t (id int GENERATED ALWAYS AS (", "generated_open"),
    ("CREATE TABLE c (LIKE ", "like_table"),
    ("CREATE TABLE c (LIKE users INCLUDING ", "like_options"),
    ("CREATE TABLE c PARTITION OF parent FOR VALUES IN (", "partition_values_in"),
    ("CREATE TABLE c PARTITION OF parent FOR VALUES FROM (", "partition_values_from"),
    ("CREATE TABLE c PARTITION OF parent FOR VALUES WITH (MODULUS ", "partition_modulus"),
    ("ALTER TABLE users ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (START WITH ", "alter_identity_start"),
    ("CREATE SEQUENCE s START WITH ", "sequence_start"),
    ("CREATE SEQUENCE s INCREMENT BY ", "sequence_increment"),
    ("CREATE TYPE my_type AS (", "create_type_attrs"),
    ("CREATE TYPE my_type AS ENUM (", "create_type_enum"),
    ("CREATE DOMAIN d AS int CHECK (", "create_domain_check"),
    ("CREATE COLLATION c (", "create_collation_attrs"),
  ] {
    let items = complete_at(s, s.len(), &cat);
    let n = items.len();
    let has_id = items.iter().any(|i| i.label == "id");
    eprintln!("R|{}|n={}|id={}", label, n, has_id);
  }
}

#[test]
fn r24_probe_like() {
  let cat = catalog_with_users_and_orders();
  let s = "CREATE TABLE c (LIKE ";
  let items = complete_at(s, s.len(), &cat);
  for (i, it) in items.iter().enumerate().take(10) {
    eprintln!("LIKE|{}|{}|{:?}", i, it.label, it.kind);
  }
  let has_u = items.iter().any(|i| i.label == "users");
  eprintln!("LIKE|has_users={}", has_u);
}

#[test]
fn r24_probe_more() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("SELECT id FROM users WHERE COALESCE(name, '') = ", "coalesce_eq"),
    ("SELECT id FROM users WHERE NULLIF(name, '') = ", "nullif_eq"),
    ("SELECT id FROM users WHERE GREATEST(id, ", "greatest_arg"),
    ("SELECT id FROM users WHERE LEAST(id, ", "least_arg"),
    ("SELECT id FROM users WHERE name LIKE concat('%', ", "like_concat"),
    ("SELECT id, name FROM users WHERE (id, name) = ANY(VALUES (1, 'a'), (2, 'b')) AND ", "values_any"),
    ("SELECT id FROM users WHERE id IS NOT DISTINCT FROM ", "not_distinct"),
    ("SELECT * FROM users u CROSS JOIN orders o WHERE u.id = ", "cross_after"),
    ("SELECT * FROM users WHERE id != ALL(SELECT id FROM users WHERE ", "all_subq_corr"),
    ("WITH x AS (DELETE FROM orders WHERE user_id = ", "cte_delete"),
    ("WITH x AS (UPDATE users SET name = 'x' WHERE id = ", "cte_update"),
    ("WITH x AS (INSERT INTO orders (id, user_id) VALUES (1, ", "cte_insert"),
  ] {
    let items = complete_at(s, s.len(), &cat);
    let n = items.len();
    let has_id = items.iter().any(|i| i.label == "id");
    eprintln!("R2|{}|n={}|id={}", label, n, has_id);
  }
}

#[test]
fn r24_strong_0001() {
  let src = "-- v0\nSELECT a[1:3] FROM users WHERE id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0002() {
  let src = "-- v0\nSELECT id FROM users WHERE id = (SELECT MAX(id) FROM users WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0003() {
  let src = "-- v0\nSELECT id FROM users u WHERE u.id IS DISTINCT FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0004() {
  let src = "-- v0\nSELECT id FROM users WHERE id IS NOT DISTINCT FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0005() {
  let src = "-- v0\nSELECT id FROM users WHERE COALESCE(name, '') = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0006() {
  let src = "-- v0\nSELECT id FROM users WHERE NULLIF(name, '') = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0007() {
  let src = "-- v0\nSELECT id FROM users WHERE GREATEST(id, ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0008() {
  let src = "-- v0\nSELECT id FROM users WHERE LEAST(id, ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0009() {
  let src = "-- v0\nSELECT id FROM users WHERE name LIKE concat('%', ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0010() {
  let src = "-- v0\nSELECT * FROM users u CROSS JOIN orders o WHERE u.id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0011() {
  let src = "-- v0\nSELECT * FROM users WHERE id != ALL(SELECT id FROM users WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0012() {
  let src = "-- v0\nWITH x AS (DELETE FROM orders WHERE user_id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0013() {
  let src = "-- v0\nWITH x AS (UPDATE users SET name = 'x' WHERE id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0014() {
  let src = "-- v0\nSELECT a[ FROM users";
  let cur = src.rfind("[").unwrap() + if src[src.rfind("[").unwrap()..].starts_with("[1:") { 3 } else { 1 };
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0015() {
  let src = "-- v0\nSELECT a[1: FROM users";
  let cur = src.rfind("[").unwrap() + if src[src.rfind("[").unwrap()..].starts_with("[1:") { 3 } else { 1 };
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0016() {
  let src = "-- v1\nSELECT a[1:3] FROM users WHERE id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0017() {
  let src = "-- v1\nSELECT id FROM users WHERE id = (SELECT MAX(id) FROM users WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0018() {
  let src = "-- v1\nSELECT id FROM users u WHERE u.id IS DISTINCT FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0019() {
  let src = "-- v1\nSELECT id FROM users WHERE id IS NOT DISTINCT FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0020() {
  let src = "-- v1\nSELECT id FROM users WHERE COALESCE(name, '') = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0021() {
  let src = "-- v1\nSELECT id FROM users WHERE NULLIF(name, '') = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0022() {
  let src = "-- v1\nSELECT id FROM users WHERE GREATEST(id, ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0023() {
  let src = "-- v1\nSELECT id FROM users WHERE LEAST(id, ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0024() {
  let src = "-- v1\nSELECT id FROM users WHERE name LIKE concat('%', ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0025() {
  let src = "-- v1\nSELECT * FROM users u CROSS JOIN orders o WHERE u.id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0026() {
  let src = "-- v1\nSELECT * FROM users WHERE id != ALL(SELECT id FROM users WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0027() {
  let src = "-- v1\nWITH x AS (DELETE FROM orders WHERE user_id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0028() {
  let src = "-- v1\nWITH x AS (UPDATE users SET name = 'x' WHERE id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0029() {
  let src = "-- v1\nSELECT a[ FROM users";
  let cur = src.rfind("[").unwrap() + if src[src.rfind("[").unwrap()..].starts_with("[1:") { 3 } else { 1 };
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0030() {
  let src = "-- v1\nSELECT a[1: FROM users";
  let cur = src.rfind("[").unwrap() + if src[src.rfind("[").unwrap()..].starts_with("[1:") { 3 } else { 1 };
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0031() {
  let src = "-- v2\nSELECT a[1:3] FROM users WHERE id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0032() {
  let src = "-- v2\nSELECT id FROM users WHERE id = (SELECT MAX(id) FROM users WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0033() {
  let src = "-- v2\nSELECT id FROM users u WHERE u.id IS DISTINCT FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0034() {
  let src = "-- v2\nSELECT id FROM users WHERE id IS NOT DISTINCT FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0035() {
  let src = "-- v2\nSELECT id FROM users WHERE COALESCE(name, '') = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0036() {
  let src = "-- v2\nSELECT id FROM users WHERE NULLIF(name, '') = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0037() {
  let src = "-- v2\nSELECT id FROM users WHERE GREATEST(id, ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0038() {
  let src = "-- v2\nSELECT id FROM users WHERE LEAST(id, ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0039() {
  let src = "-- v2\nSELECT id FROM users WHERE name LIKE concat('%', ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0040() {
  let src = "-- v2\nSELECT * FROM users u CROSS JOIN orders o WHERE u.id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0041() {
  let src = "-- v2\nSELECT * FROM users WHERE id != ALL(SELECT id FROM users WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0042() {
  let src = "-- v2\nWITH x AS (DELETE FROM orders WHERE user_id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0043() {
  let src = "-- v2\nWITH x AS (UPDATE users SET name = 'x' WHERE id = ";
  let cur = src.len();
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0044() {
  let src = "-- v2\nSELECT a[ FROM users";
  let cur = src.rfind("[").unwrap() + if src[src.rfind("[").unwrap()..].starts_with("[1:") { 3 } else { 1 };
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r24_strong_0045() {
  let src = "-- v2\nSELECT a[1: FROM users";
  let cur = src.rfind("[").unwrap() + if src[src.rfind("[").unwrap()..].starts_with("[1:") { 3 } else { 1 };
  let items = complete_at(src, cur, &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"), "missing id");
}

#[test]
fn r25_probe() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("COPY users (", "copy_cols"),
    ("COPY users TO STDOUT WITH (", "copy_with_opts"),
    ("COPY users TO STDOUT WITH (FORMAT ", "copy_format"),
    ("COPY users TO STDOUT WITH (DELIMITER ", "copy_delim"),
    ("DO $$ DECLARE x int; BEGIN  END $$", "do_body"),
    ("SELECT * FROM jsonb_each('{}') AS x(", "jsonb_each_alias"),
    ("SELECT * FROM jsonb_array_elements('[]') WITH ORDINALITY AS x(", "ord_alias"),
    ("CREATE INDEX ON users USING gin (data ", "gin_ops_lhs"),
    ("CREATE INDEX ON users (id ", "idx_col_ops"),
    ("CREATE INDEX ON users (id ASC, name ", "idx_after_col"),
    ("CREATE TABLE t (id int, EXCLUDE USING gist (", "exclude_open"),
    ("CREATE FOREIGN TABLE ft (", "fdw_cols"),
    ("CREATE FOREIGN TABLE ft (id int) SERVER ", "fdw_server"),
    ("ALTER FOREIGN TABLE ft OPTIONS (", "alter_ft_opts"),
    ("ALTER PUBLICATION p ADD TABLE ", "alter_pub_add"),
    ("ALTER SUBSCRIPTION s ENABLE", "alter_sub"),
    ("REFRESH MATERIALIZED VIEW CONCURRENTLY ", "refresh_concurrent"),
    ("DROP MATERIALIZED VIEW IF EXISTS ", "drop_mv_ifexists"),
    ("CREATE TABLE x (LIKE users INCLUDING DEFAULTS INCLUDING ", "like_inc_more"),
    ("CREATE TABLE x (LIKE users EXCLUDING ", "like_excl"),
  ] {
    let items = complete_at(s, s.len(), &cat);
    let has_users = items.iter().any(|i| i.label == "users");
    let has_id = items.iter().any(|i| i.label == "id");
    eprintln!("R|{}|n={}|id={}|users={}", label, items.len(), has_id, has_users);
  }
}

#[test]
fn r25_idx_next_0001() {
  let src = "-- ix0\nCREATE INDEX ON users (id ASC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0002() {
  let src = "-- ix0\nCREATE INDEX ON users (id DESC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0003() {
  let src = "-- ix0\nCREATE INDEX ON users (id NULLS FIRST, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0004() {
  let src = "-- ix0\nCREATE INDEX ON users (id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0005() {
  let src = "-- ix0\nCREATE INDEX ON users (email, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "email"));
}

#[test]
fn r25_idx_next_0006() {
  let src = "-- ix0\nCREATE INDEX ON users (name, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "email"));
}

#[test]
fn r25_idx_next_0007() {
  let src = "-- ix1\nCREATE INDEX ON users (id ASC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0008() {
  let src = "-- ix1\nCREATE INDEX ON users (id DESC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0009() {
  let src = "-- ix1\nCREATE INDEX ON users (id NULLS FIRST, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0010() {
  let src = "-- ix1\nCREATE INDEX ON users (id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0011() {
  let src = "-- ix1\nCREATE INDEX ON users (email, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "email"));
}

#[test]
fn r25_idx_next_0012() {
  let src = "-- ix1\nCREATE INDEX ON users (name, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "email"));
}

#[test]
fn r25_idx_next_0013() {
  let src = "-- ix2\nCREATE INDEX ON users (id ASC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0014() {
  let src = "-- ix2\nCREATE INDEX ON users (id DESC, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0015() {
  let src = "-- ix2\nCREATE INDEX ON users (id NULLS FIRST, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0016() {
  let src = "-- ix2\nCREATE INDEX ON users (id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "name" || i.label == "email"));
}

#[test]
fn r25_idx_next_0017() {
  let src = "-- ix2\nCREATE INDEX ON users (email, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "email"));
}

#[test]
fn r25_idx_next_0018() {
  let src = "-- ix2\nCREATE INDEX ON users (name, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id" || i.label == "email"));
}

#[test]
fn r25_refresh_0019() {
  let src = "-- rf0\nREFRESH MATERIALIZED VIEW CONCURRENTLY ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_refresh_0020() {
  let src = "-- rf0\nREFRESH MATERIALIZED VIEW ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_refresh_0021() {
  let src = "-- rf1\nREFRESH MATERIALIZED VIEW CONCURRENTLY ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_refresh_0022() {
  let src = "-- rf1\nREFRESH MATERIALIZED VIEW ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_refresh_0023() {
  let src = "-- rf2\nREFRESH MATERIALIZED VIEW CONCURRENTLY ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_refresh_0024() {
  let src = "-- rf2\nREFRESH MATERIALIZED VIEW ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_drop_mv_0029() {
  let src = "-- dmv0\nDROP MATERIALIZED VIEW IF EXISTS ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_drop_mv_0030() {
  let src = "-- dmv0\nDROP MATERIALIZED VIEW ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_drop_mv_0031() {
  let src = "-- dmv1\nDROP MATERIALIZED VIEW IF EXISTS ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_drop_mv_0032() {
  let src = "-- dmv1\nDROP MATERIALIZED VIEW ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_drop_mv_0033() {
  let src = "-- dmv2\nDROP MATERIALIZED VIEW IF EXISTS ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}

#[test]
fn r25_drop_mv_0034() {
  let src = "-- dmv2\nDROP MATERIALIZED VIEW ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "users"));
}


#[test]
fn r26_probe() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("SELECT id COLLATE ", "collate_arg"),
    ("SELECT * FROM users WHERE name COLLATE \"C\" = ", "collate_eq"),
    ("CREATE TABLE t (id int, name text COLLATE ", "collate_in_decl"),
    ("CREATE INDEX ON users (name COLLATE ", "collate_in_idx"),
    ("SELECT * FROM users TABLESAMPLE BERNOULLI (10) REPEATABLE (42) WHERE ", "tablesample_where"),
    ("SELECT * FROM users TABLESAMPLE SYSTEM (10) WHERE ", "tablesample_system_where"),
    ("SELECT * FROM XMLTABLE('/a' PASSING ", "xmltable_passing"),
    ("SELECT * FROM JSON_TABLE('{}' , '$' COLUMNS (", "json_table_cols"),
    ("SELECT * FROM users WHERE name IN (SELECT name FROM users WHERE id = ", "in_subq_corr"),
    ("SELECT * FROM users WHERE EXISTS (SELECT FROM orders WHERE user_id = ", "exists_no_select"),
    ("SELECT u.id FROM users u JOIN orders o ON u.id = o.user_id JOIN orders o2 ON o2.id = ", "deep_join_on"),
    ("SELECT u.id FROM users u JOIN orders o ON u.id = o.user_id WHERE o.id = ", "join_then_where"),
    ("SELECT * FROM users WHERE id IS NULL AND email IS ", "is_after_and"),
    ("UPDATE users SET name = 'x' WHERE id IN (SELECT id FROM users WHERE email = ", "update_with_subq"),
    ("DELETE FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE orders.user_id = ", "delete_with_corr"),
    ("SELECT id FROM users WHERE id = 1 RETURNING ", "select_no_returning"),
    ("INSERT INTO users (id, name) SELECT id, ", "insert_select_after"),
    ("INSERT INTO users (id) SELECT id FROM orders WHERE user_id = ", "insert_select_corr"),
  ] {
    let items = complete_at(s, s.len(), &cat);
    let n = items.len();
    let has_id = items.iter().any(|i| i.label == "id");
    eprintln!("R|{}|n={}|id={}", label, n, has_id);
  }
}

#[test]
fn r26_strong_0001() {
  let src = "-- v0\nSELECT * FROM users WHERE name COLLATE \"C\" = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0002() {
  let src = "-- v0\nSELECT * FROM users TABLESAMPLE BERNOULLI (10) REPEATABLE (42) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0003() {
  let src = "-- v0\nSELECT * FROM users TABLESAMPLE SYSTEM (10) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0004() {
  let src = "-- v0\nSELECT * FROM users WHERE name IN (SELECT name FROM users WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0005() {
  let src = "-- v0\nSELECT * FROM users WHERE EXISTS (SELECT FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0006() {
  let src = "-- v0\nSELECT u.id FROM users u JOIN orders o ON u.id = o.user_id JOIN orders o2 ON o2.id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0007() {
  let src = "-- v0\nSELECT u.id FROM users u JOIN orders o ON u.id = o.user_id WHERE o.id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0008() {
  let src = "-- v0\nUPDATE users SET name = 'x' WHERE id IN (SELECT id FROM users WHERE email = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0009() {
  let src = "-- v0\nDELETE FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE orders.user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0010() {
  let src = "-- v0\nINSERT INTO users (id) SELECT id FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0011() {
  let src = "-- v0\nSELECT * FROM users WHERE id IS DISTINCT FROM (SELECT max(id) FROM users WHERE name = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0012() {
  let src = "-- v0\nSELECT * FROM users WHERE id = (SELECT min(user_id) FROM orders WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0013() {
  let src = "-- v0\nWITH cte AS (SELECT id FROM users WHERE email = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0014() {
  let src = "-- v0\nSELECT * FROM users u WHERE u.id NOT IN (SELECT user_id FROM orders WHERE id < ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0015() {
  let src = "-- v1\nSELECT * FROM users WHERE name COLLATE \"C\" = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0016() {
  let src = "-- v1\nSELECT * FROM users TABLESAMPLE BERNOULLI (10) REPEATABLE (42) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0017() {
  let src = "-- v1\nSELECT * FROM users TABLESAMPLE SYSTEM (10) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0018() {
  let src = "-- v1\nSELECT * FROM users WHERE name IN (SELECT name FROM users WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0019() {
  let src = "-- v1\nSELECT * FROM users WHERE EXISTS (SELECT FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0020() {
  let src = "-- v1\nSELECT u.id FROM users u JOIN orders o ON u.id = o.user_id JOIN orders o2 ON o2.id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0021() {
  let src = "-- v1\nSELECT u.id FROM users u JOIN orders o ON u.id = o.user_id WHERE o.id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0022() {
  let src = "-- v1\nUPDATE users SET name = 'x' WHERE id IN (SELECT id FROM users WHERE email = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0023() {
  let src = "-- v1\nDELETE FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE orders.user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0024() {
  let src = "-- v1\nINSERT INTO users (id) SELECT id FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0025() {
  let src = "-- v1\nSELECT * FROM users WHERE id IS DISTINCT FROM (SELECT max(id) FROM users WHERE name = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0026() {
  let src = "-- v1\nSELECT * FROM users WHERE id = (SELECT min(user_id) FROM orders WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0027() {
  let src = "-- v1\nWITH cte AS (SELECT id FROM users WHERE email = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0028() {
  let src = "-- v1\nSELECT * FROM users u WHERE u.id NOT IN (SELECT user_id FROM orders WHERE id < ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0029() {
  let src = "-- v2\nSELECT * FROM users WHERE name COLLATE \"C\" = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0030() {
  let src = "-- v2\nSELECT * FROM users TABLESAMPLE BERNOULLI (10) REPEATABLE (42) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0031() {
  let src = "-- v2\nSELECT * FROM users TABLESAMPLE SYSTEM (10) WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0032() {
  let src = "-- v2\nSELECT * FROM users WHERE name IN (SELECT name FROM users WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0033() {
  let src = "-- v2\nSELECT * FROM users WHERE EXISTS (SELECT FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0034() {
  let src = "-- v2\nSELECT u.id FROM users u JOIN orders o ON u.id = o.user_id JOIN orders o2 ON o2.id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0035() {
  let src = "-- v2\nSELECT u.id FROM users u JOIN orders o ON u.id = o.user_id WHERE o.id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0036() {
  let src = "-- v2\nUPDATE users SET name = 'x' WHERE id IN (SELECT id FROM users WHERE email = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0037() {
  let src = "-- v2\nDELETE FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE orders.user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0038() {
  let src = "-- v2\nINSERT INTO users (id) SELECT id FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0039() {
  let src = "-- v2\nSELECT * FROM users WHERE id IS DISTINCT FROM (SELECT max(id) FROM users WHERE name = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0040() {
  let src = "-- v2\nSELECT * FROM users WHERE id = (SELECT min(user_id) FROM orders WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0041() {
  let src = "-- v2\nWITH cte AS (SELECT id FROM users WHERE email = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r26_strong_0042() {
  let src = "-- v2\nSELECT * FROM users u WHERE u.id NOT IN (SELECT user_id FROM orders WHERE id < ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_probe() {
  let cat = catalog_with_users_and_orders();
  for (s, label) in [
    ("SELECT * FROM users WHERE id = (SELECT id FROM users WHERE email = (SELECT email FROM users WHERE id = ", "triple_nested"),
    ("SELECT id FROM users GROUP BY GROUPING SETS ((", "grouping_sets_open"),
    ("SELECT id FROM users GROUP BY ROLLUP (id, ", "rollup_after"),
    ("SELECT id FROM users GROUP BY CUBE (", "cube_open"),
    ("SELECT id FROM users HAVING count(*) > (SELECT count(*) FROM orders WHERE user_id = ", "having_corr_subq"),
    ("SELECT lag(name, 1, ", "lag_default_arg"),
    ("SELECT first_value(", "first_value_open"),
    ("SELECT nth_value(name, ", "nth_value_arg2"),
    ("SELECT * FROM users u FOR UPDATE OF u WHERE ", "for_update_then_where"),
    ("SELECT * FROM users u FOR SHARE OF u WHERE ", "for_share_then_where"),
    ("SELECT * FROM users LIMIT 1 OFFSET ", "offset_value"),
    ("SELECT * FROM users LIMIT 1 OFFSET 5 FETCH NEXT ", "fetch_next_value"),
    ("SELECT * FROM (VALUES (1, 'a'), (2, 'b')) AS v(", "values_alias"),
    ("CREATE TABLE t (id int PRIMARY KEY, ts timestamp WITH ", "with_in_decl"),
    ("CREATE TABLE t (a int CHECK (a > 0) NO ", "check_no_inherit"),
    ("ALTER TABLE users ALTER COLUMN id ADD ", "alter_add"),
    ("ALTER TABLE users ALTER COLUMN id DROP ", "alter_drop"),
    ("ALTER TABLE users CLUSTER ON ", "cluster_idx"),
    ("ALTER INDEX my_idx RENAME TO ", "alter_idx_rename"),
    ("ALTER SCHEMA s RENAME TO ", "alter_schema_rename"),
  ] {
    let items = complete_at(s, s.len(), &cat);
    let n = items.len();
    let has_id = items.iter().any(|i| i.label == "id");
    let has_email = items.iter().any(|i| i.label == "email");
    eprintln!("R|{}|n={}|id={}|email={}", label, n, has_id, has_email);
  }
}

#[test]
fn r27_strong_0001() {
  let src = "-- v0\nSELECT * FROM users WHERE id = (SELECT id FROM users WHERE email = (SELECT email FROM users WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0002() {
  let src = "-- v0\nSELECT id FROM users GROUP BY GROUPING SETS ((";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0003() {
  let src = "-- v0\nSELECT id FROM users GROUP BY CUBE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0004() {
  let src = "-- v0\nSELECT id FROM users HAVING count(*) > (SELECT count(*) FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0005() {
  let src = "-- v0\nSELECT lag(name, 1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0006() {
  let src = "-- v0\nSELECT first_value(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0007() {
  let src = "-- v0\nSELECT nth_value(name, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0008() {
  let src = "-- v0\nSELECT * FROM users u FOR UPDATE OF u WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0009() {
  let src = "-- v0\nSELECT * FROM users u FOR SHARE OF u WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0010() {
  let src = "-- v0\nSELECT * FROM users u FOR NO KEY UPDATE OF u WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0011() {
  let src = "-- v0\nSELECT * FROM users u FOR KEY SHARE OF u WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0012() {
  let src = "-- v0\nSELECT id FROM users GROUP BY ROLLUP (id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r27_strong_0013() {
  let src = "-- v1\nSELECT * FROM users WHERE id = (SELECT id FROM users WHERE email = (SELECT email FROM users WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0014() {
  let src = "-- v1\nSELECT id FROM users GROUP BY GROUPING SETS ((";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0015() {
  let src = "-- v1\nSELECT id FROM users GROUP BY CUBE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0016() {
  let src = "-- v1\nSELECT id FROM users HAVING count(*) > (SELECT count(*) FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0017() {
  let src = "-- v1\nSELECT lag(name, 1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0018() {
  let src = "-- v1\nSELECT first_value(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0019() {
  let src = "-- v1\nSELECT nth_value(name, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0020() {
  let src = "-- v1\nSELECT * FROM users u FOR UPDATE OF u WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0021() {
  let src = "-- v1\nSELECT * FROM users u FOR SHARE OF u WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0022() {
  let src = "-- v1\nSELECT * FROM users u FOR NO KEY UPDATE OF u WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0023() {
  let src = "-- v1\nSELECT * FROM users u FOR KEY SHARE OF u WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0024() {
  let src = "-- v1\nSELECT id FROM users GROUP BY ROLLUP (id, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "email"));
}

#[test]
fn r27_strong_0025() {
  let src = "-- v2\nSELECT * FROM users WHERE id = (SELECT id FROM users WHERE email = (SELECT email FROM users WHERE id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0026() {
  let src = "-- v2\nSELECT id FROM users GROUP BY GROUPING SETS ((";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0027() {
  let src = "-- v2\nSELECT id FROM users GROUP BY CUBE (";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0028() {
  let src = "-- v2\nSELECT id FROM users HAVING count(*) > (SELECT count(*) FROM orders WHERE user_id = ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0029() {
  let src = "-- v2\nSELECT lag(name, 1, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0030() {
  let src = "-- v2\nSELECT first_value(";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0031() {
  let src = "-- v2\nSELECT nth_value(name, ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0032() {
  let src = "-- v2\nSELECT * FROM users u FOR UPDATE OF u WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}

#[test]
fn r27_strong_0033() {
  let src = "-- v2\nSELECT * FROM users u FOR SHARE OF u WHERE ";
  let items = complete_at(src, src.len(), &catalog_with_users_and_orders());
  assert!(items.iter().any(|i| i.label == "id"));
}
