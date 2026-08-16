//! Batch 7 of the SQL rule expansion plan (sql784-sql786): GROUPING
//! SETS/CUBE/ROLLUP depth. See
//! dsl-analysis/docs/plans/2026-08-18-grouping-sets-rules.md.

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
fn sql784_duplicate_set() {
  let d = diags("SELECT a, b, count(*) FROM t784 GROUP BY GROUPING SETS ((a,b), (a,b));");
  assert!(d.iter().any(|x| x.code == "sql784" && x.severity == Severity::Hint));
}

#[test]
fn sql784_quiet_distinct_sets() {
  let d = diags("SELECT a, b, count(*) FROM t784 GROUP BY GROUPING SETS ((a,b), (a));");
  assert!(!d.iter().any(|x| x.code == "sql784"));
}

#[test]
fn sql785_arg_not_in_group_by() {
  let d = diags("SELECT a, b, GROUPING(c) FROM t785 GROUP BY a, b;");
  assert!(d.iter().any(|x| x.code == "sql785" && x.severity == Severity::Error));
}

#[test]
fn sql785_quiet_when_arg_in_group_by() {
  let d = diags("SELECT a, b, GROUPING(a) FROM t785 GROUP BY a, b;");
  assert!(!d.iter().any(|x| x.code == "sql785"));
}

#[test]
fn sql786_duplicate_in_rollup() {
  let d = diags("SELECT a, count(*) FROM t786b GROUP BY ROLLUP (a, a);");
  assert!(d.iter().any(|x| x.code == "sql786" && x.severity == Severity::Warning));
}

#[test]
fn sql786_duplicate_in_cube() {
  let d = diags("SELECT a, count(*) FROM t786b GROUP BY CUBE (a, a);");
  assert!(d.iter().any(|x| x.code == "sql786" && x.severity == Severity::Warning));
}

#[test]
fn sql786_quiet_distinct_columns() {
  let d = diags("SELECT a, b, count(*) FROM t786b GROUP BY ROLLUP (a, b);");
  assert!(!d.iter().any(|x| x.code == "sql786"));
}
