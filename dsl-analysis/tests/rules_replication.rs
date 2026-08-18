//! Batch 11 of the SQL rule expansion plan (sql795-sql797): logical
//! replication mistakes. See
//! dsl-analysis/docs/plans/2026-08-18-logical-replication-rules.md.

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
fn sql795_duplicate_schema() {
  let d = diags("CREATE PUBLICATION p795b FOR TABLES IN SCHEMA s795, s795;");
  assert!(d.iter().any(|x| x.code == "sql795" && x.severity == Severity::Error));
}

#[test]
fn sql795_quiet_distinct_schemas() {
  let d = diags("CREATE PUBLICATION p795bg FOR TABLES IN SCHEMA s795, s795b;");
  assert!(!d.iter().any(|x| x.code == "sql795"));
}

#[test]
fn sql796_create_slot_false_no_slot_name() {
  let d = diags("CREATE SUBSCRIPTION sub796 CONNECTION 'dbname=foo' PUBLICATION pub796 WITH (create_slot = false);");
  assert!(d.iter().any(|x| x.code == "sql796" && x.severity == Severity::Error));
}

#[test]
fn sql796_quiet_when_slot_name_given() {
  let d = diags(
    "CREATE SUBSCRIPTION sub796g CONNECTION 'dbname=foo' PUBLICATION pub796g WITH (create_slot = false, slot_name = 'myslot');",
  );
  assert!(!d.iter().any(|x| x.code == "sql796"));
}

#[test]
fn sql797_duplicate_table() {
  let d = diags("CREATE PUBLICATION p797 FOR TABLE t797a, t797a;");
  assert!(d.iter().any(|x| x.code == "sql797" && x.severity == Severity::Error));
}

#[test]
fn sql797_quiet_distinct_tables() {
  let d = diags("CREATE PUBLICATION p797g FOR TABLE t797a, t797b;");
  assert!(!d.iter().any(|x| x.code == "sql797"));
}
