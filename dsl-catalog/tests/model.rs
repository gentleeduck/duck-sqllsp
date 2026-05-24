use dsl_catalog::{Catalog, Column, Schema, Table, TableKind, CATALOG_VERSION};

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
                }],
                constraints: vec![],
                indexes: vec![], triggers: vec![], policies: vec![],
                comment: None,
            }],
        }],
        functions: vec![],
        types: vec![],
    }
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
