//! Batch 1 of the SQL rule expansion plan (sql757-sql762): declarative
//! table partitioning DDL mistakes. See
//! dsl-analysis/docs/plans/2026-08-18-partitioning-ddl-rules.md.

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
fn sql757_pk_missing_partition_column() {
  let d = diags(
    "CREATE TABLE t757a (id int, created_at date, PRIMARY KEY (id)) PARTITION BY RANGE (created_at);",
  );
  assert!(d.iter().any(|x| x.code == "sql757" && x.severity == Severity::Error));
}

#[test]
fn sql757_quiet_when_pk_covers_partition_column() {
  let d = diags(
    "CREATE TABLE t757b (id int, created_at date, PRIMARY KEY (id, created_at)) PARTITION BY RANGE (created_at);",
  );
  assert!(!d.iter().any(|x| x.code == "sql757"));
}

#[test]
fn sql757_quiet_when_not_partitioned() {
  let d = diags("CREATE TABLE t757c (id int, PRIMARY KEY (id));");
  assert!(!d.iter().any(|x| x.code == "sql757"));
}

#[test]
fn sql758_reversed_bound() {
  let d = diags("ALTER TABLE t758 ATTACH PARTITION t758_bad FOR VALUES FROM (100) TO (1);");
  assert!(d.iter().any(|x| x.code == "sql758" && x.severity == Severity::Error));
}

#[test]
fn sql758_quiet_when_ascending() {
  let d = diags("ALTER TABLE t758 ATTACH PARTITION t758_good FOR VALUES FROM (1) TO (100);");
  assert!(!d.iter().any(|x| x.code == "sql758"));
}

#[test]
fn sql759_volatile_partition_expr() {
  let d = diags("CREATE TABLE t759a (id int, created_at timestamptz) PARTITION BY RANGE (now());");
  assert!(d.iter().any(|x| x.code == "sql759" && x.severity == Severity::Error));
}

#[test]
fn sql759_quiet_on_bare_column() {
  let d = diags("CREATE TABLE t759b (id int, created_at timestamptz) PARTITION BY RANGE (created_at);");
  assert!(!d.iter().any(|x| x.code == "sql759"));
}

#[test]
fn sql760_duplicate_partition_column() {
  let d = diags("CREATE TABLE t760a (a int, b int) PARTITION BY RANGE (a, a);");
  assert!(d.iter().any(|x| x.code == "sql760" && x.severity == Severity::Warning));
}

#[test]
fn sql760_quiet_when_distinct() {
  let d = diags("CREATE TABLE t760b (a int, b int) PARTITION BY RANGE (a, b);");
  assert!(!d.iter().any(|x| x.code == "sql760"));
}

#[test]
fn sql761_detach_concurrently_in_tx() {
  let d = diags("BEGIN;\nALTER TABLE t761 DETACH PARTITION t761_bad CONCURRENTLY;\nCOMMIT;");
  assert!(d.iter().any(|x| x.code == "sql761" && x.severity == Severity::Error));
}

#[test]
fn sql761_quiet_outside_tx() {
  let d = diags("ALTER TABLE t761 DETACH PARTITION t761_good CONCURRENTLY;");
  assert!(!d.iter().any(|x| x.code == "sql761"));
}

#[test]
fn sql762_remainder_not_less_than_modulus() {
  let d = diags("ALTER TABLE t762 ATTACH PARTITION t762_bad FOR VALUES WITH (MODULUS 4, REMAINDER 4);");
  assert!(d.iter().any(|x| x.code == "sql762" && x.severity == Severity::Error));
}

#[test]
fn sql762_quiet_when_remainder_less_than_modulus() {
  let d = diags("ALTER TABLE t762 ATTACH PARTITION t762_good FOR VALUES WITH (MODULUS 4, REMAINDER 3);");
  assert!(!d.iter().any(|x| x.code == "sql762"));
}
