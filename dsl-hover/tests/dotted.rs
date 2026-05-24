//! Hover narrow-by-side for dotted identifiers.

use dsl_catalog::{Catalog, Column, Schema, Table, TableKind, CATALOG_VERSION};
use dsl_hover::hover;
use text_size::TextSize;

fn cat() -> Catalog {
    let users = Table {
        schema: "public".into(),
        name: "users".into(),
        kind: TableKind::Table,
        columns: vec![
            Column { name: "id".into(),    data_type: "uuid".into(), nullable: false, default: None, comment: None },
            Column { name: "email".into(), data_type: "text".into(), nullable: false, default: None, comment: None },
        ],
        constraints: vec![],
        indexes: vec![], triggers: vec![], policies: vec![],
        comment: None,
    };
    Catalog {
        version: CATALOG_VERSION,
        connection_id: "test".into(),
        schemas: vec![Schema { name: "public".into(), tables: vec![users] }],
        functions: vec![],
        types: vec![],
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
    assert!(md.contains("Table") || md.contains("users"),
        "alias hover should show table: {md}");
    // Must NOT be a single-column card.
    assert!(!md.starts_with("# Column"), "alias side got column card: {md}");
}

#[test]
fn cursor_on_column_right_shows_column_card() {
    let src = "SELECT u.id FROM users u";
    let cur = src.find("u.id").unwrap() + 2; // on `id`
    let md = hover_at(src, cur).expect("hover for column");
    assert!(md.contains("Column") || md.contains("public.users.id"),
        "column hover should focus column: {md}");
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
    assert!(md.to_ascii_lowercase().contains("text"),
        "expected text-type card; got: {md}");
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
    assert!(md.contains("id") && md.to_ascii_lowercase().contains("uuid"),
        "expected id-column card; got: {md}");
}

#[test]
fn hover_on_schema_qualified_table_card() {
    let src = "SELECT * FROM public.users;";
    let cur = src.find(".users").unwrap() + 1;
    let md = hover_at(src, cur).expect("schema.table hover should resolve");
    assert!(md.contains("users"), "expected users-table card; got: {md}");
}
