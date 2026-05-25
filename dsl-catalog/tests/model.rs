use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Extension, Schema, Sequence, Table, TableKind};

fn sample() -> Catalog {
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema {
      name: "public".into(),
      tables: vec![Table {
        schema: "public".into(),
        name: "users".into(),
        kind: TableKind::Table,
        columns: vec![Column {
          name: "id".into(),
          data_type: "uuid".into(),
          nullable: false,
          default: Some("gen_random_uuid()".into()),
          comment: None,
          generated: None,
          json_keys: None,
        }],
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
    sequences: vec![Sequence {
      schema: "public".into(),
      name: "users_id_seq".into(),
      data_type: "bigint".into(),
      start_value: 1,
      min_value: 1,
      max_value: i64::MAX,
      increment_by: 1,
      cycle: false,
      owned_by_column: Some("public.users.id".into()),
      comment: None,
    }],
    extensions: vec![Extension {
      name: "pgcrypto".into(),
      schema: "public".into(),
      version: "1.3".into(),
      comment: Some("cryptographic functions".into()),
    }],
  }
}

#[test]
fn finds_sequence_by_name() {
  let cat = sample();
  let s = cat.find_sequence(None, "users_id_seq").unwrap();
  assert_eq!(s.schema, "public");
  assert_eq!(s.data_type, "bigint");
}

#[test]
fn has_extension_is_case_insensitive() {
  let cat = sample();
  assert!(cat.has_extension("pgcrypto"));
  assert!(cat.has_extension("PGCRYPTO"));
  assert!(!cat.has_extension("postgis"));
}

#[test]
fn old_catalog_json_round_trips_without_new_fields() {
  // Catalogs cached before sequences/extensions existed must still
  // deserialise (serde(default)).
  let json = r#"{
      "version": 1, "connection_id": "x", "schemas": [],
      "functions": [], "types": [], "roles": []
    }"#;
  let cat: Catalog = serde_json::from_str(json).unwrap();
  assert!(cat.sequences.is_empty());
  assert!(cat.extensions.is_empty());
}

#[test]
fn round_trips_through_json() {
  let cat = sample();
  let json = serde_json::to_string(&cat).unwrap();
  let back: Catalog = serde_json::from_str(&json).unwrap();
  assert_eq!(back.connection_id, "test");
  assert_eq!(back.tables().count(), 1);
}

#[test]
fn find_table_matches_unqualified() {
  let cat = sample();
  let t = cat.find_table(None, "users").unwrap();
  assert_eq!(t.name, "users");
}

#[test]
fn find_table_matches_qualified() {
  let cat = sample();
  assert!(cat.find_table(Some("public"), "users").is_some());
  assert!(cat.find_table(Some("nope"), "users").is_none());
}

#[test]
fn columns_named_locates_ambiguous_columns() {
  let cat = sample();
  let hits = cat.columns_named("id");
  assert_eq!(hits.len(), 1);
  assert_eq!(hits[0].0.name, "users");
}
