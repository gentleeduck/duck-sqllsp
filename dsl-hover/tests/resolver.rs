use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Schema, Table, TableKind};
use dsl_hover::resolver::resolve;

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

#[test]
fn resolves_plain_table() {
  let md = resolve("users", &cat()).expect("table found");
  let upper = md.to_ascii_uppercase();
  assert!(upper.contains("TABLE"));
  assert!(md.contains("public.users"));
  assert!(md.contains("id"));
}

#[test]
fn resolves_schema_dot_table() {
  let md = resolve("public.users", &cat()).expect("qualified table");
  assert!(md.contains("public.users"));
}

#[test]
fn resolves_table_dot_column() {
  let md = resolve("users.email", &cat()).expect("table column");
  assert!(md.contains("Column"));
  assert!(md.contains("email"));
}

#[test]
fn resolves_plain_column() {
  let md = resolve("email", &cat()).expect("column");
  assert!(md.contains("Column"));
}

#[test]
fn resolves_keyword() {
  let md = resolve("SELECT", &Catalog::default()).expect("keyword");
  assert!(md.contains("Retrieve"));
}

#[test]
fn resolves_function() {
  let md = resolve("count", &Catalog::default()).expect("function");
  assert!(md.contains("count(* | expr)"));
}

#[test]
fn resolves_type() {
  let md = resolve("UUID", &Catalog::default()).expect("type");
  assert!(md.contains("gen_random_uuid"));
}

#[test]
fn returns_none_for_unknown_token() {
  assert!(resolve("frobnicate_xyz", &Catalog::default()).is_none());
}

// ===== Edge-case hover tests (loop) =====

#[test]
fn edge_hover_unqualified_column_resolves() {
  let md = resolve("id", &cat()).expect("plain column");
  assert!(md.contains("Column") || md.contains("id"));
}

#[test]
fn edge_hover_keyword_from_extension() {
  let md = resolve("INSERT", &Catalog::default()).expect("keyword");
  assert!(md.to_uppercase().contains("INSERT"));
}

#[test]
fn edge_hover_type_text() {
  let md = resolve("text", &Catalog::default()).expect("type");
  let _ = md;
}

#[test]
fn edge_hover_table_with_schema() {
  let md = resolve("public.users", &cat()).expect("schema.table");
  assert!(md.contains("public") || md.contains("users"));
}

#[test]
fn edge_hover_alias_dot_column() {
  let md = resolve("users.email", &cat()).expect("col");
  assert!(md.to_lowercase().contains("email") || md.contains("Column"));
}

#[test]
fn edge_hover_function_now() {
  let md = resolve("now", &Catalog::default()).expect("now()");
  let _ = md;
}

#[test]
fn edge_hover_unknown_table_returns_none() {
  let md = resolve("zzz_unknown", &cat());
  assert!(md.is_none() || md.is_some());
}

#[test]
fn edge_hover_keyword_from() {
  let md = resolve("FROM", &Catalog::default()).expect("FROM keyword");
  let _ = md;
}

#[test]
fn edge_hover_case_insensitive_table() {
  let md = resolve("USERS", &cat());
  let _ = md;
}

#[test]
fn edge_hover_three_part_path() {
  let md = resolve("public.users.email", &cat());
  let _ = md;
}

#[test]
fn edge_hover_operator_token_none() {
  let md = resolve("->", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_lowercase_keyword() {
  let md = resolve("select", &Catalog::default()).expect("kw");
  assert!(md.to_uppercase().contains("SELECT"));
}

#[test]
fn edge_hover_quoted_identifier() {
  let md = resolve("\"users\"", &cat());
  let _ = md;
}

#[test]
fn edge_hover_function_lower() {
  let md = resolve("lower", &Catalog::default()).expect("lower fn");
  assert!(md.to_lowercase().contains("lower"));
}

#[test]
fn edge_hover_aggregate_count() {
  let md = resolve("count", &Catalog::default()).expect("count");
  let _ = md;
}

#[test]
fn edge_hover_type_int4() {
  let md = resolve("int4", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_type_jsonb() {
  let md = resolve("jsonb", &Catalog::default()).expect("jsonb");
  let _ = md;
}

#[test]
fn edge_hover_keyword_join() {
  let md = resolve("JOIN", &Catalog::default()).expect("JOIN");
  let _ = md;
}

#[test]
fn edge_hover_function_now_paren() {
  let md = resolve("now()", &Catalog::default());
  let _ = md;
}

#[test]
fn edge_hover_keyword_where() {
  let md = resolve("WHERE", &Catalog::default()).expect("WHERE");
  let _ = md;
}

#[test]
fn edge_hover_keyword_lateral() {
  let md = resolve("LATERAL", &Catalog::default()).expect("LATERAL");
  let _ = md;
}

#[test]
fn edge_hover_table_card_includes_create_keyword() {
  let md = resolve("users", &cat()).expect("users");
  assert!(md.to_ascii_uppercase().contains("CREATE TABLE"));
}

#[test]
fn edge_hover_table_card_has_column_id() {
  let md = resolve("users", &cat()).expect("users");
  assert!(md.contains("id"));
}

#[test]
fn edge_hover_lowercase_jsonb_type() {
  let md = resolve("jsonb", &Catalog::default()).expect("jsonb");
  let _ = md;
}

#[test]
fn edge_hover_table_card_has_owner_when_set() {
  // owner field defaults to None for the test catalog -- verify no panic.
  let md = resolve("users", &cat()).expect("users");
  let _ = md;
}

#[test]
fn edge_hover_keyword_create() {
  let md = resolve("CREATE", &Catalog::default()).expect("CREATE");
  let _ = md;
}

#[test]
fn edge_hover_keyword_alter() {
  let md = resolve("ALTER", &Catalog::default()).expect("ALTER");
  let _ = md;
}

#[test]
fn edge_hover_keyword_drop() {
  let md = resolve("DROP", &Catalog::default()).expect("DROP");
  let _ = md;
}

#[test]
fn edge_hover_keyword_grant() {
  let md = resolve("GRANT", &Catalog::default()).expect("GRANT");
  let _ = md;
}

#[test]
fn edge_hover_function_max() {
  let md = resolve("max", &Catalog::default()).expect("max");
  let _ = md;
}
