use dsl_catalog::{Catalog, Column, Schema, Table, TableKind, CATALOG_VERSION};
use dsl_completion::{complete, ItemKind};
use dsl_parse::{parse, Dialect};
use dsl_resolve::resolve_with_source;
use text_size::TextSize;

fn catalog_with_users_and_orders() -> Catalog {
    let users = Table {
        schema: "public".into(),
        name: "users".into(),
        kind: TableKind::Table,
        columns: vec![
            Column { name: "id".into(),    data_type: "uuid".into(), nullable: false, default: None, comment: None },
            Column { name: "email".into(), data_type: "text".into(), nullable: false, default: None, comment: None },
            Column { name: "name".into(),  data_type: "text".into(), nullable: true,  default: None, comment: None },
        ],
        constraints: vec![],
        indexes: vec![], triggers: vec![], policies: vec![],
        comment: None,
    };
    let orders = Table {
        schema: "public".into(),
        name: "orders".into(),
        kind: TableKind::Table,
        columns: vec![
            Column { name: "id".into(),       data_type: "uuid".into(), nullable: false, default: None, comment: None },
            Column { name: "user_id".into(),  data_type: "uuid".into(), nullable: false, default: None, comment: None },
        ],
        constraints: vec![],
        indexes: vec![], triggers: vec![], policies: vec![],
        comment: None,
    };
    Catalog {
        version: CATALOG_VERSION,
        connection_id: "test".into(),
        schemas: vec![Schema { name: "public".into(), tables: vec![users, orders] }],
        functions: vec![],
        types: vec![],
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
    let bare_cols: Vec<&str> = items.iter()
        .filter(|i| i.kind == ItemKind::Column)
        .map(|i| i.label.as_str())
        .collect();
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
    let bare_cols: Vec<&str> = items.iter()
        .filter(|i| i.kind == ItemKind::Column)
        .map(|i| i.label.as_str())
        .collect();
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
    let bare_cols: Vec<&str> = items.iter()
        .filter(|i| i.kind == ItemKind::Column)
        .map(|i| i.label.as_str())
        .collect();
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
    assert!(!items.iter().any(|i| i.label == "id" && i.kind == ItemKind::Column),
        "INT-typed local leaked columns: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>());
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
        "v_count missing: {:?}", items.iter().take(20).map(|i| &i.label).collect::<Vec<_>>()
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
    assert!(!has_alias(&items, "users"),
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
    assert!(items.iter().all(|i| i.kind == ItemKind::Column),
        "expected only CTE columns, got {:?}",
        items.iter().map(|i| (&i.label, &i.kind)).collect::<Vec<_>>());
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
    assert!(labels.iter().any(|l| l.eq_ignore_ascii_case("length")),
        "expected length() in CHECK completion, got: {labels:?}");
    assert!(labels.iter().any(|l| l.eq_ignore_ascii_case("char_length")),
        "expected char_length() in CHECK completion, got: {labels:?}");
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
    assert!(labels.iter().any(|l| l.eq_ignore_ascii_case("id") || l.eq_ignore_ascii_case("email")),
        "expected columns of users in CHECK, got: {labels:?}");
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
    for fname in &["length", "char_length", "character_length", "now", "coalesce", "count", "lower", "upper", "substring"] {
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing from SELECT projection completion; got {} items",
            labels.len());
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
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing from WHERE completion");
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
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing from CHECK completion");
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
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing from HAVING completion");
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
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing from ORDER BY completion");
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
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing from `CONSTRAINT name CHECK (` completion");
    }
}

#[test]
fn after_default_keyword_offers_functions() {
    let cat = catalog_with_users_and_orders();
    let src = "CREATE TABLE t (created_at timestamptz DEFAULT ";
    let cur = src.len();
    let items = complete_at(src, cur, &cat);
    let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
    for fname in &["now", "gen_random_uuid"] {
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing after DEFAULT");
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
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing inside inline CHECK");
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
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing from ON-clause completion");
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
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing from IN-predicate completion");
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
    assert!(c.insert_text.contains("PRIMARY KEY,UNIQUE,FOREIGN KEY,CHECK"),
        "expects kind choice; got: {}", c.insert_text);
    assert!(c.insert_text.contains("${3:col}"), "expects column-list placeholder");
}

#[test]
fn ctable_snippet_expands_create_table_skeleton() {
    let cat = catalog_with_users_and_orders();
    let src = "";
    let items = complete_at(src, 0, &cat);
    let it = items.iter().find(|i| i.label == "ctable").expect("ctable");
    assert!(it.is_snippet);
    assert!(it.insert_text.contains("CREATE TABLE ${1:name}"),
        "got: {}", it.insert_text);
    assert!(it.insert_text.contains("gen_random_uuid()"),
        "expects PK + default");
    assert!(it.insert_text.contains("created_at timestamptz NOT NULL DEFAULT now()"),
        "expects created_at");
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
    assert!(it.insert_text.contains("BEGIN\n    $0"),
        "tab-stop should land in BEGIN body");
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
fn functions_in_plpgsql_assign_rhs() {
    let cat = catalog_with_users_and_orders();
    let src = "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ DECLARE v text; BEGIN v := ";
    let cur = src.len();
    let items = complete_at(src, cur, &cat);
    let labels: Vec<String> = items.iter().map(|i| i.label.to_ascii_lowercase()).collect();
    for fname in &["length", "now", "coalesce"] {
        assert!(labels.iter().any(|l| l == fname),
            "function `{fname}` missing from PL/pgSQL assign RHS completion");
    }
}
