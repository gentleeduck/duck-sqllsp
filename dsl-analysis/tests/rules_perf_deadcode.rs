//! Batch 14 (final) of the SQL rule expansion plan (sql803-sql804):
//! loop-body RAISE NOTICE and unused PL/pgSQL variables. See
//! dsl-analysis/docs/plans/2026-08-18-perf-deadcode-rules.md.

use dsl_analysis::{Severity, run};
use dsl_catalog::{CATALOG_VERSION, Catalog, Schema};
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

fn diags(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, &empty_cat())
}

#[test]
fn sql803_raise_notice_in_loop() {
  let d = diags("DO $$ BEGIN FOR i IN 1..10 LOOP RAISE NOTICE 'processing %', i; END LOOP; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql803" && x.severity == Severity::Hint));
}

#[test]
fn sql803_quiet_without_raise_notice() {
  let d = diags("DO $$ BEGIN FOR i IN 1..10 LOOP PERFORM i; END LOOP; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql803"));
}

#[test]
fn sql804_unused_variable() {
  let d = diags("DO $$ DECLARE unused_var804 int; BEGIN RAISE NOTICE 'hello'; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql804" && x.severity == Severity::Hint));
}

#[test]
fn sql804_quiet_when_used() {
  let d = diags("DO $$ DECLARE used_var804 int; BEGIN used_var804 := 5; RAISE NOTICE '%', used_var804; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql804"));
}
