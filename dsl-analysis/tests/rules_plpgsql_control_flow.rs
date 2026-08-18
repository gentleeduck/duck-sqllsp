//! Batch 12 of the SQL rule expansion plan (sql798-sql800): PL/pgSQL
//! loop and exception control-flow mistakes. See
//! dsl-analysis/docs/plans/2026-08-18-plpgsql-control-flow-rules.md.

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

fn cat_with_id_column() -> Catalog {
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema {
      name: "public".into(),
      tables: vec![Table {
        schema: "public".into(),
        name: "t799".into(),
        kind: TableKind::Table,
        columns: vec![Column {
          name: "id".into(),
          data_type: "int".into(),
          nullable: false,
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
fn sql798_bare_loop_no_exit() {
  let d = diags("DO $$ BEGIN LOOP PERFORM 1; END LOOP; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql798" && x.severity == Severity::Warning));
}

#[test]
fn sql798_quiet_with_exit() {
  let d = diags("DO $$ BEGIN LOOP EXIT WHEN true; END LOOP; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql798"));
}

#[test]
fn sql799_loop_var_shadows_column() {
  let d = diags_with_cat(
    "DO $$ BEGIN FOR id IN SELECT generate_series(1,3) LOOP RAISE NOTICE '%', id; END LOOP; END; $$;",
    &cat_with_id_column(),
  );
  assert!(d.iter().any(|x| x.code == "sql799" && x.severity == Severity::Hint));
}

#[test]
fn sql799_quiet_without_catalog() {
  let d = diags("DO $$ BEGIN FOR id IN SELECT generate_series(1,3) LOOP RAISE NOTICE '%', id; END LOOP; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql799"));
}

#[test]
fn sql800_null_only_handler() {
  let d = diags("DO $$ BEGIN RAISE EXCEPTION 'test'; EXCEPTION WHEN OTHERS THEN NULL; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql800" && x.severity == Severity::Warning));
}

#[test]
fn sql800_quiet_with_real_handling() {
  let d = diags("DO $$ BEGIN RAISE EXCEPTION 'test'; EXCEPTION WHEN OTHERS THEN RAISE NOTICE 'caught: %', SQLERRM; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql800"));
}
