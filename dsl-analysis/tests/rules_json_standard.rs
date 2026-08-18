//! Batch 2 of the SQL rule expansion plan (sql763-sql768): SQL-standard
//! JSON function misuse. See
//! dsl-analysis/docs/plans/2026-08-18-json-standard-rules.md.

use dsl_analysis::{Severity, run};
use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Schema, Table, TableKind};
use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve_with_source;

fn empty_cat() -> Catalog {
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema { name: "public".into(), tables: vec![] }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

fn cat_with_jsonb_col() -> Catalog {
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema {
      name: "public".into(),
      tables: vec![Table {
        schema: "public".into(),
        name: "t767".into(),
        kind: TableKind::Table,
        columns: vec![Column {
          name: "doc".into(),
          data_type: "jsonb".into(),
          nullable: true,
          default: None,
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
        owner: None,
        definition: None,
        strict: false,
        options: None,
      }],
    }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

fn diags(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  diags_with_cat(src, &empty_cat())
}

fn diags_with_cat(src: &str, cat: &Catalog) -> Vec<dsl_analysis::Diagnostic> {
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, cat)
}

#[test]
fn sql763_bad_path_no_dollar() {
  let d = diags("SELECT JSON_EXISTS(doc, 'not-a-path') FROM t763;");
  assert!(d.iter().any(|x| x.code == "sql763" && x.severity == Severity::Error));
}

#[test]
fn sql763_quiet_on_valid_path() {
  let d = diags("SELECT JSON_EXISTS(doc, '$.a') FROM t763;");
  assert!(!d.iter().any(|x| x.code == "sql763"));
}

#[test]
fn sql764_returning_without_on_error() {
  let d = diags("SELECT JSON_VALUE(doc, '$.a' RETURNING int) FROM t764;");
  assert!(d.iter().any(|x| x.code == "sql764" && x.severity == Severity::Hint));
}

#[test]
fn sql764_quiet_with_on_error() {
  let d = diags("SELECT JSON_VALUE(doc, '$.a' RETURNING int NULL ON ERROR) FROM t764;");
  assert!(!d.iter().any(|x| x.code == "sql764"));
}

#[test]
fn sql765_wrapper_omit_quotes_conflict() {
  let d = diags("SELECT JSON_QUERY(doc, '$.a' WITH WRAPPER OMIT QUOTES) FROM t765;");
  assert!(d.iter().any(|x| x.code == "sql765" && x.severity == Severity::Error));
}

#[test]
fn sql765_quiet_wrapper_alone() {
  let d = diags("SELECT JSON_QUERY(doc, '$.a' WITH WRAPPER) FROM t765;");
  assert!(!d.iter().any(|x| x.code == "sql765"));
}

#[test]
fn sql766_duplicate_output_column() {
  let d = diags(
    "SELECT * FROM t766, JSON_TABLE(doc, '$[*]' COLUMNS (a int PATH '$.a', a text PATH '$.b')) AS jt;",
  );
  assert!(d.iter().any(|x| x.code == "sql766" && x.severity == Severity::Warning));
}

#[test]
fn sql766_quiet_when_distinct() {
  let d = diags(
    "SELECT * FROM t766, JSON_TABLE(doc, '$[*]' COLUMNS (a int PATH '$.a', b text PATH '$.b')) AS jt;",
  );
  assert!(!d.iter().any(|x| x.code == "sql766"));
}

#[test]
fn sql767_redundant_on_jsonb_column() {
  let d = diags_with_cat("SELECT doc FROM t767 WHERE doc IS JSON;", &cat_with_jsonb_col());
  assert!(d.iter().any(|x| x.code == "sql767" && x.severity == Severity::Hint));
}

#[test]
fn sql767_quiet_without_catalog() {
  // No catalog info for `doc` -- rule must stay quiet, not guess.
  let d = diags("SELECT doc FROM t767 WHERE doc IS JSON;");
  assert!(!d.iter().any(|x| x.code == "sql767"));
}

#[test]
fn sql767_quiet_on_is_json_object() {
  // Narrower IS JSON OBJECT/ARRAY/SCALAR checks are out of scope.
  let d = diags_with_cat("SELECT doc FROM t767 WHERE doc IS JSON OBJECT;", &cat_with_jsonb_col());
  assert!(!d.iter().any(|x| x.code == "sql767"));
}

#[test]
fn sql768_object_and_array_conflict() {
  let d = diags("SELECT doc FROM t768 WHERE doc IS JSON OBJECT AND doc IS JSON ARRAY;");
  assert!(d.iter().any(|x| x.code == "sql768" && x.severity == Severity::Warning));
}

#[test]
fn sql768_quiet_single_check() {
  let d = diags("SELECT doc FROM t768 WHERE doc IS JSON OBJECT;");
  assert!(!d.iter().any(|x| x.code == "sql768"));
}
