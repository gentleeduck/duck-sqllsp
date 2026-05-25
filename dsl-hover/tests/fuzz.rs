//! Fuzz-style hover tests: feed many SQL snippets at every cursor
//! position and assert `hover()` never panics.

use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Schema, Table, TableKind};
use dsl_hover::hover;
use text_size::TextSize;

fn cat() -> Catalog {
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema {
      name: "public".into(),
      tables: vec![Table {
        schema: "public".into(),
        name: "users".into(),
        kind: TableKind::Table,
        columns: vec![
          Column { name: "id".into(), data_type: "uuid".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
          Column { name: "email".into(), data_type: "text".into(), nullable: false, default: None, comment: None, generated: None, json_keys: None },
        ],
        constraints: vec![],
        indexes: vec![],
        triggers: vec![],
        policies: vec![],
        comment: None,
    row_estimate: None,
      }],
    }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

const FIXTURES: &[&str] = &[
  "",
  "SELECT u.id FROM users u",
  "SELECT users.id FROM users",
  "INSERT INTO users (id, email) VALUES ('00000000-0000-0000-0000-000000000000', 'a')",
  "UPDATE users SET email = 'x' WHERE id = '00000000-0000-0000-0000-000000000000'",
  "DELETE FROM users WHERE id = '00000000-0000-0000-0000-000000000000'",
  "CREATE TABLE foo (id INT PRIMARY KEY, name TEXT)",
  "CREATE OR REPLACE FUNCTION f() RETURNS INT LANGUAGE plpgsql AS $$ BEGIN RETURN 1; END; $$;",
  "WITH x AS (SELECT 1) SELECT * FROM x",
  "SELECT now()::DATE",
];

#[test]
fn fuzz_hover_never_panics_at_every_cursor() {
  let c = cat();
  for src in FIXTURES {
    for pos in 0..=src.len() {
      let _ = hover(src, TextSize::from(pos as u32), &c);
    }
  }
}

#[test]
fn fuzz_hover_pathological_inputs() {
  let c = cat();
  let cases =
    ["/* never closed", "'unterminated string", "$$ unclosed dollar", "....", "a.b.c.d.e.f", "((((((((", ";;;;;;;;"];
  for src in &cases {
    for pos in 0..=src.len() {
      let _ = hover(src, TextSize::from(pos as u32), &c);
    }
  }
}

#[test]
fn fuzz_hover_on_keywords_returns_doc_or_none() {
  let c = cat();
  // Hovering over SELECT / FROM / WHERE / etc. should return some
  // markdown or None -- never a panic, never an empty Ok.
  for kw in &["SELECT", "FROM", "WHERE", "GROUP", "ORDER", "LIMIT", "JOIN", "ON", "AS"] {
    let src = format!("{kw} 1");
    for pos in 0..src.len() {
      if let Some(md) = hover(&src, TextSize::from(pos as u32), &c) {
        assert!(!md.is_empty(), "hover returned empty string for `{src}` at {pos}");
      }
    }
  }
}
