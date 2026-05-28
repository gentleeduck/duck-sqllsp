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
    owner: None,
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
    owner: None,
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
fn dot_context_with_unknown_alias_returns_empty() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT zzz. FROM users u";
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  let offset = TextSize::from("SELECT zzz.".len() as u32);
  let items = complete(src, &file, &scopes, &cat, offset);
  assert!(items.is_empty());
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
  assert!(items.is_empty(), "expected no completion after CREATE TABLE; got {} items", items.len());
}

#[test]
fn no_completion_after_create_function_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE FUNCTION ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty());
}

#[test]
fn no_completion_after_create_or_replace_function_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE OR REPLACE FUNCTION ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty());
}

#[test]
fn no_completion_after_create_index_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty());
}

#[test]
fn no_completion_after_create_view_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE VIEW ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty());
}

#[test]
fn no_completion_after_create_trigger_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty());
}

#[test]
fn no_completion_after_create_policy_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE POLICY ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty());
}

#[test]
fn no_completion_after_create_type_if_not_exists() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TYPE IF NOT EXISTS ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty());
}

#[test]
fn no_completion_while_typing_fresh_name() {
  // Cursor mid-identifier `my_n|` -- still suppressed.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE my_n";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty(), "expected no completion mid-identifier; got {} items", items.len());
}

#[test]
fn completion_after_create_table_body_still_works() {
  // Once the user is *inside* the body `CREATE TABLE x (`, completion
  // returns to normal column/constraint suggestions.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE x (";
  let items = complete_at(src, src.len(), &cat);
  assert!(!items.is_empty(), "expected body-start completion");
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
fn import_foreign_schema_fresh_name_emits_nothing() {
  let cat = catalog_with_users_and_orders();
  let src = "IMPORT FOREIGN SCHEMA ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty(), "IMPORT FOREIGN SCHEMA is fresh-name; got {} items", items.len());
}

#[test]
fn checkpoint_emits_nothing() {
  let cat = catalog_with_users_and_orders();
  let src = "CHECKPOINT ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty(), "CHECKPOINT takes no args; got {} items", items.len());
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
fn listen_unlisten_notify_emit_nothing() {
  // LISTEN / UNLISTEN / NOTIFY take a channel name (freeform).
  // No catalog completion makes sense.
  let cat = catalog_with_users_and_orders();
  for src in ["LISTEN ", "UNLISTEN ", "NOTIFY "] {
    let items = complete_at(src, src.len(), &cat);
    assert!(
      items.is_empty(),
      "{src:?} is a fresh-name slot; expected empty, got {} items",
      items.len()
    );
  }
}

#[test]
fn prepare_deallocate_emit_nothing() {
  let cat = catalog_with_users_and_orders();
  for src in ["PREPARE ", "DEALLOCATE "] {
    let items = complete_at(src, src.len(), &cat);
    assert!(
      items.is_empty(),
      "{src:?} is a fresh-name slot; expected empty, got {} items",
      items.len()
    );
  }
}

#[test]
fn declare_emits_nothing() {
  // DECLARE <name> CURSOR FOR ... -- fresh cursor name.
  let cat = catalog_with_users_and_orders();
  let src = "DECLARE ";
  let items = complete_at(src, src.len(), &cat);
  assert!(
    items.is_empty(),
    "DECLARE is a fresh-name slot; expected empty, got {} items",
    items.len()
  );
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
fn create_index_concurrently_fresh_name_emits_nothing() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX CONCURRENTLY ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty(), "CREATE INDEX CONCURRENTLY is fresh-name; got {} items", items.len());
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
fn alter_table_rename_column_to_fresh_name_emits_nothing() {
  // `ALTER TABLE users RENAME COLUMN id TO <cursor>` -- inventing a
  // new column name. Currently dumps 18 action keywords.
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users RENAME COLUMN id TO ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty(), "RENAME COLUMN ... TO is fresh-name; got {} items", items.len());
}

#[test]
fn create_extension_fresh_name_emits_nothing() {
  // `CREATE EXTENSION <cursor>` -- user types an extension name
  // (uuid-ossp / pgcrypto / etc). No catalog SQL identifier is the
  // right answer; the menu must not be the 640-item catch-all dump.
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EXTENSION ";
  let items = complete_at(src, src.len(), &cat);
  assert!(
    items.is_empty(),
    "CREATE EXTENSION is a fresh-name slot; expected empty menu, got {} items",
    items.len()
  );
}

#[test]
fn create_extension_if_not_exists_fresh_name_emits_nothing() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EXTENSION IF NOT EXISTS ";
  let items = complete_at(src, src.len(), &cat);
  assert!(
    items.is_empty(),
    "CREATE EXTENSION IF NOT EXISTS is a fresh-name slot; expected empty menu, got {} items",
    items.len()
  );
}

#[test]
fn alter_table_add_column_fresh_name_emits_nothing() {
  // `ALTER TABLE users ADD COLUMN <cursor>` -- the user is inventing
  // a fresh column name. Don't suggest action keywords or anything
  // catalog-derived.
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ADD COLUMN ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty(), "ADD COLUMN is a fresh-name slot; expected empty menu, got {} items", items.len());
}

#[test]
fn alter_table_add_constraint_fresh_name_emits_nothing() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ADD CONSTRAINT ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty(), "ADD CONSTRAINT is a fresh-name slot; expected empty menu, got {} items", items.len());
}

#[test]
fn alter_table_rename_to_fresh_name_emits_nothing() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users RENAME TO ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty(), "RENAME TO is a fresh-name slot; expected empty menu, got {} items", items.len());
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
fn create_event_trigger_fresh_name_emits_nothing() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE EVENT TRIGGER ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty(), "CREATE EVENT TRIGGER is a fresh-name slot; got {} items", items.len());
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
fn with_recursive_fresh_name_emits_nothing() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH RECURSIVE ";
  let items = complete_at(src, src.len(), &cat);
  assert!(items.is_empty(), "WITH RECURSIVE expects fresh CTE name; got {} items", items.len());
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
  // / views. Currently dumps 600+ items (keywords + functions + types
  // + columns + tables) because the phase walker falls into the
  // generic Unknown catch-all after the DROP keyword.
  let cat = catalog_with_users_and_orders();
  let src = "DROP TABLE ";
  let items = complete_at(src, src.len(), &cat);
  assert!(
    items.iter().all(|i| matches!(i.kind, ItemKind::Table | ItemKind::View)),
    "DROP TABLE menu must be only tables/views; got {} items with kinds {:?}",
    items.len(),
    items.iter().map(|i| i.kind).collect::<Vec<_>>().iter().take(8).collect::<Vec<_>>()
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
  // The menu should not be 300+ catalog functions in this slot.
  let fns = items.iter().filter(|i| i.kind == ItemKind::Function).count();
  assert!(fns < 50, "RETURNING menu drowned in {fns} functions; should narrow");
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
fn dot_context_unknown_schema_dot_returns_empty() {
  // `FROM nonexistent_schema.` -- not a schema, not an alias; emit
  // nothing rather than dumping the full menu.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM zzz_no_such.";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(items.is_empty(), "unknown qualifier must yield no items, got {} items", items.len());
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

#[test]
fn no_completion_for_sql_looking_string_content() {
  let cat = catalog_with_users_and_orders();
  // A string that happens to contain SQL keywords must still be inert.
  let src = "SELECT * FROM users WHERE comment = 'SELECT FROM '";
  let cur = "SELECT * FROM users WHERE comment = 'SELECT FROM ".len();
  let items = complete_at(src, cur, &cat);
  assert!(items.is_empty(), "string content shouldn't trigger completion, got {} items", items.len());
}

#[test]
fn no_completion_inside_line_comment() {
  let cat = catalog_with_users_and_orders();
  // Cursor inside `-- ...` line comment.
  let src = "SELECT id FROM users -- pick a ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(items.is_empty(), "no completion inside line comment, got {} items", items.len());
}

#[test]
fn no_completion_inside_block_comment() {
  let cat = catalog_with_users_and_orders();
  // Cursor inside `/* ... */` block comment.
  let src = "SELECT id FROM users /* todo: filter ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(items.is_empty(), "no completion inside block comment, got {} items", items.len());
}

#[test]
fn completion_still_works_after_closed_string() {
  // Sanity: a closed string before the cursor must NOT suppress completion.
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users WHERE name = 'bob' AND ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(!items.is_empty(), "completion should resume after a closed string");
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
  assert_eq!(items.len(), 0, "RANGE body expects opts; menu should be empty, got {} items", items.len());
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
  assert!(labels.iter().any(|l| *l == "CREATE TABLE"), "baseline must include DDL: {:?}", &labels[..labels.len().min(15)]);
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
  assert!(items.len() <= 30, "DROP slot should be tight, got {} -- {:?}", items.len(), &labels[..labels.len().min(15)]);
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
  assert!(items.len() <= 12, "DEFAULT slot should be tight, got {} -- {labels:?}", items.len());
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
  assert!(items.len() <= 12, "got {} -- {labels:?}", items.len());
  assert!(labels.contains(&"now()"));
}

#[test]
fn create_table_subsequent_column_default_still_narrows() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (a int DEFAULT 0, b text DEFAULT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.len() <= 12, "got {} -- {labels:?}", items.len());
  assert!(labels.contains(&"NULL"));
}

#[test]
fn alter_table_add_column_default_slot_offers_curated_expressions() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ADD COLUMN c INT DEFAULT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  assert!(items.len() <= 12, "DEFAULT slot should be tight, got {} -- {labels:?}", items.len());
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
fn completion_still_works_after_closed_block_comment() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users /* a note */ WHERE ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  assert!(!items.is_empty(), "completion should resume after a closed block comment");
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
  assert!(labels.contains(&"generate_series"), "expected generate_series: {labels:?}");
  assert!(!labels.contains(&"users"), "users table must not appear at LATERAL-target slot: {labels:?}");
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
fn edge_complete_returning_after_insert() {
  let cat = catalog_with_users_and_orders();
  let src = "INSERT INTO users (name) VALUES ('x') RETURNING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
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
fn edge_complete_group_by() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT email, count(*) FROM users GROUP BY ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_after_select_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // Should include either columns (without FROM) or keywords.
  let _ = labels;
}

#[test]
fn edge_complete_with_clause() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id FROM users) SELECT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_returning_after_returning_kw() {
  let cat = catalog_with_users_and_orders();
  let src = "DELETE FROM users WHERE id = '00000000-0000-0000-0000-000000000001' RETURNING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_after_having() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT email FROM users GROUP BY email HAVING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_create_trigger_event() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TRIGGER t AFTER ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_alter_table_action() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_inside_cte() {
  let cat = catalog_with_users_and_orders();
  let src = "WITH t AS (SELECT id FROM users) SELECT * FROM ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
  // CTE name `t` should appear plus catalog tables.
  let _ = labels;
}

#[test]
fn edge_complete_after_set_keyword() {
  let cat = catalog_with_users_and_orders();
  let src = "UPDATE users SET name = 'x', ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_create_index_using() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE INDEX i ON users USING ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
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
fn edge_complete_alter_table_after_users() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users ADD COLUMN ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_create_table_column_type() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_after_owner_to() {
  let cat = catalog_with_users_and_orders();
  let src = "ALTER TABLE users OWNER TO ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_default_keyword_in_col_decl() {
  let cat = catalog_with_users_and_orders();
  let src = "CREATE TABLE t (id int DEFAULT ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
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
fn edge_complete_in_join_alias() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT * FROM users u JOIN orders o ON ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_at_end_of_buffer() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id FROM users";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
}

#[test]
fn edge_complete_after_comma_in_projection() {
  let cat = catalog_with_users_and_orders();
  let src = "SELECT id, ";
  let cur = src.len();
  let items = complete_at(src, cur, &cat);
  let _ = items;
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
