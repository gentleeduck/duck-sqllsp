//! Batch 9 of the SQL rule expansion plan (sql790-sql792): statistics
//! object and NULLS NOT DISTINCT mistakes. See
//! dsl-analysis/docs/plans/2026-08-18-statistics-nulls-distinct-rules.md.

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
fn sql790_redundant_nulls_not_distinct() {
  let d = diags("CREATE TABLE t790 (id int, email text NOT NULL UNIQUE NULLS NOT DISTINCT);");
  assert!(d.iter().any(|x| x.code == "sql790" && x.severity == Severity::Hint));
}

#[test]
fn sql790_quiet_when_nullable() {
  let d = diags("CREATE TABLE t790g (id int, email text UNIQUE NULLS NOT DISTINCT);");
  assert!(!d.iter().any(|x| x.code == "sql790"));
}

#[test]
fn sql791_single_column() {
  let d = diags("CREATE STATISTICS s791 (ndistinct) ON a FROM t791;");
  assert!(d.iter().any(|x| x.code == "sql791" && x.severity == Severity::Error));
}

#[test]
fn sql791_quiet_two_columns() {
  let d = diags("CREATE STATISTICS s791g (ndistinct) ON a, b FROM t791;");
  assert!(!d.iter().any(|x| x.code == "sql791"));
}

#[test]
fn sql792_duplicate_column() {
  let d = diags("CREATE STATISTICS s792 (ndistinct) ON a, a FROM t792;");
  assert!(d.iter().any(|x| x.code == "sql792" && x.severity == Severity::Warning));
}

#[test]
fn sql792_quiet_distinct_columns() {
  let d = diags("CREATE STATISTICS s792g (ndistinct) ON a, b FROM t792;");
  assert!(!d.iter().any(|x| x.code == "sql792"));
}
