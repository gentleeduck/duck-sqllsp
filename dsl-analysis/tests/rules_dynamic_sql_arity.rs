//! Batch 13 of the SQL rule expansion plan (sql801-sql802): dynamic
//! SQL EXECUTE/USING/INTO arity mismatches. See
//! dsl-analysis/docs/plans/2026-08-18-dynamic-sql-arity-rules.md.

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
fn sql801_missing_using_arg() {
  let d = diags("DO $$ BEGIN EXECUTE 'SELECT * FROM t801 WHERE id = $1 AND x = $2' USING 5; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql801" && x.severity == Severity::Error));
}

#[test]
fn sql801_quiet_when_matching() {
  let d = diags("DO $$ BEGIN EXECUTE 'SELECT * FROM t801 WHERE id = $1 AND x = $2' USING 5, 'y'; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql801"));
}

#[test]
fn sql801_quiet_format_own_placeholders() {
  let d = diags("DO $$ BEGIN EXECUTE format('SELECT %I FROM t801 WHERE id = $1', 'col') USING 5; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql801"));
}

#[test]
fn sql802_arity_mismatch() {
  let d = diags("DO $$ DECLARE a int; b int; BEGIN EXECUTE 'SELECT 1, 2, 3' INTO a, b; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql802" && x.severity == Severity::Warning));
}

#[test]
fn sql802_quiet_when_matching() {
  let d = diags("DO $$ DECLARE a int; b int; BEGIN EXECUTE 'SELECT 1, 2' INTO a, b; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql802"));
}
