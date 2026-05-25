//! Fuzz-style completion tests: feed many SQL snippets at many cursor
//! positions; assert `complete()` never panics and respects basic
//! output invariants (dedup, kind valid).

use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Schema, Table, TableKind};
use dsl_completion::{ItemKind, complete};
use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve;
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
  "S",
  "SE",
  "SELECT",
  "SELECT ",
  "SELECT *",
  "SELECT * FROM",
  "SELECT * FROM ",
  "SELECT * FROM users",
  "SELECT id FROM users",
  "SELECT u. FROM users u",
  "SELECT now()::",
  "UPDATE users SET ",
  "INSERT INTO users (id) VALUES (",
  "WITH cte AS (SELECT 1) SELECT * FROM ",
  "CREATE TABLE foo (",
  "CREATE INDEX ix ON ",
  "CREATE TRIGGER trg BEFORE UPDATE ON ",
  "SELECT id FROM users WHERE ",
  "SELECT id FROM users ORDER BY ",
  "SELECT id FROM users GROUP BY ",
];

#[test]
fn fuzz_complete_never_panics_at_every_cursor() {
  let c = cat();
  for src in FIXTURES {
    let file = parse(src, Dialect::Postgres);
    let scopes = resolve(&file.statements);
    for pos in 0..=src.len() {
      let _ = complete(src, &file, &scopes, &c, TextSize::from(pos as u32));
    }
  }
}

#[test]
fn fuzz_complete_returns_unique_items_per_kind() {
  let c = cat();
  for src in FIXTURES {
    let file = parse(src, Dialect::Postgres);
    let scopes = resolve(&file.statements);
    let items = complete(src, &file, &scopes, &c, TextSize::from(src.len() as u32));
    let mut seen = std::collections::HashSet::new();
    for it in &items {
      let key = (it.label.to_ascii_lowercase(), it.kind);
      assert!(seen.insert(key.clone()), "duplicate (label, kind) pair {:?} in `{src}`", key);
    }
  }
}

#[test]
fn fuzz_complete_all_kinds_recognised() {
  let c = cat();
  for src in FIXTURES {
    let file = parse(src, Dialect::Postgres);
    let scopes = resolve(&file.statements);
    let items = complete(src, &file, &scopes, &c, TextSize::from(src.len() as u32));
    for it in &items {
      // ItemKind is enum-typed in source; any value is by
      // construction valid. Touch the variant so a future
      // non-exhaustive variant fails the test.
      match it.kind {
        ItemKind::Keyword
        | ItemKind::Type
        | ItemKind::Function
        | ItemKind::Table
        | ItemKind::View
        | ItemKind::Column
        | ItemKind::Schema
        | ItemKind::Variable
        | ItemKind::Parameter => {},
      }
    }
  }
}

#[test]
fn fuzz_unicode_input_never_panics() {
  let c = cat();
  let cases = [
    "SELECT é FROM çafé",
    "SELECT αβγ FROM δεζ",
    "SELECT 你好 FROM 用户",
    "SELECT \"emoji_🔥\" FROM users",
    "SELECT a FROM users -- ñoño",
    "SELECT 'مرحبا' FROM users",
  ];
  for src in &cases {
    let file = parse(src, Dialect::Postgres);
    let scopes = resolve(&file.statements);
    // Walk only valid UTF-8 char boundaries -- LSP clients send
    // positions in code units, never inside a multi-byte sequence.
    for (pos, _) in src.char_indices() {
      let _ = complete(src, &file, &scopes, &c, TextSize::from(pos as u32));
    }
    // End of string too.
    let _ = complete(src, &file, &scopes, &c, TextSize::from(src.len() as u32));
  }
}

#[test]
fn fuzz_pathological_input_never_panics() {
  let c = cat();
  let cases = [
    "/* unterminated",
    "SELECT 'unterminated",
    "SELECT $$ no end",
    ";;;;;",
    "SELECT ((((((",
    "SELECT u.u.u.u.u.u.u",
    ".",
    "..",
    "...",
    "WHERE",
  ];
  for src in &cases {
    let file = parse(src, Dialect::Postgres);
    let scopes = resolve(&file.statements);
    for pos in 0..=src.len() {
      let _ = complete(src, &file, &scopes, &c, TextSize::from(pos as u32));
    }
  }
}
