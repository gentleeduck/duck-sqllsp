//! Batch 5 of the SQL rule expansion plan (sql777-sql779): domain and
//! composite type mistakes. See
//! dsl-analysis/docs/plans/2026-08-18-domain-composite-rules.md.

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
fn sql777_check_missing_value() {
  let d = diags("CREATE DOMAIN age777 AS int CHECK (1 > 0);");
  assert!(d.iter().any(|x| x.code == "sql777" && x.severity == Severity::Warning));
}

#[test]
fn sql777_quiet_when_value_referenced() {
  let d = diags("CREATE DOMAIN age777g AS int CHECK (VALUE > 0);");
  assert!(!d.iter().any(|x| x.code == "sql777"));
}

#[test]
fn sql778_default_violates_check() {
  let d = diags("CREATE DOMAIN age778 AS int CHECK (VALUE > 0) DEFAULT -1;");
  assert!(d.iter().any(|x| x.code == "sql778" && x.severity == Severity::Warning));
}

#[test]
fn sql778_quiet_when_default_satisfies_check() {
  let d = diags("CREATE DOMAIN age778g AS int CHECK (VALUE > 0) DEFAULT 5;");
  assert!(!d.iter().any(|x| x.code == "sql778"));
}

#[test]
fn sql779_duplicate_field() {
  let d = diags("CREATE TYPE t779 AS (a int, a text);");
  assert!(d.iter().any(|x| x.code == "sql779" && x.severity == Severity::Error));
}

#[test]
fn sql779_quiet_distinct_fields() {
  let d = diags("CREATE TYPE t779g AS (a int, b text);");
  assert!(!d.iter().any(|x| x.code == "sql779"));
}
