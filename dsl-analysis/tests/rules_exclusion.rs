//! Batch 4 of the SQL rule expansion plan (sql774-sql776): EXCLUDE
//! constraint mistakes. See
//! dsl-analysis/docs/plans/2026-08-18-exclusion-constraint-rules.md.

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
fn sql774_duplicate_column() {
  let d = diags("CREATE TABLE t774b (id int, during tsrange, EXCLUDE USING gist (during WITH &&, during WITH &&));");
  assert!(d.iter().any(|x| x.code == "sql774" && x.severity == Severity::Warning));
}

#[test]
fn sql774_quiet_distinct_columns() {
  let d =
    diags("CREATE TABLE t774bg (id int, during tsrange, room int, EXCLUDE USING gist (room WITH =, during WITH &&));");
  assert!(!d.iter().any(|x| x.code == "sql774"));
}

#[test]
fn sql775_btree_unsupported() {
  let d = diags("CREATE TABLE t775 (id int, during tsrange, EXCLUDE USING btree (during WITH =));");
  assert!(d.iter().any(|x| x.code == "sql775" && x.severity == Severity::Error));
}

#[test]
fn sql775_quiet_gist() {
  let d = diags("CREATE TABLE t775g (id int, during tsrange, EXCLUDE USING gist (during WITH &&));");
  assert!(!d.iter().any(|x| x.code == "sql775"));
}

#[test]
fn sql776_single_column_eq() {
  let d = diags("CREATE TABLE t776 (id int, EXCLUDE USING gist (id WITH =));");
  assert!(d.iter().any(|x| x.code == "sql776" && x.severity == Severity::Hint));
}

#[test]
fn sql776_quiet_multi_column() {
  let d = diags("CREATE TABLE t776g (id int, during tsrange, EXCLUDE USING gist (id WITH =, during WITH &&));");
  assert!(!d.iter().any(|x| x.code == "sql776"));
}
