//! Batch 10 of the SQL rule expansion plan (sql793-sql794): MERGE
//! branch reachability and scope mistakes. See
//! dsl-analysis/docs/plans/2026-08-18-merge-depth-rules.md.

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
fn sql793_unconditional_before_conditioned() {
  let d = diags(
    "MERGE INTO t793 USING s793 ON t793.id = s793.id WHEN MATCHED THEN UPDATE SET x = s793.x WHEN MATCHED AND s793.y > 0 THEN DELETE;",
  );
  assert!(d.iter().any(|x| x.code == "sql793" && x.severity == Severity::Warning));
}

#[test]
fn sql793_quiet_when_conditioned_first() {
  let d = diags(
    "MERGE INTO t793g USING s793g ON t793g.id = s793g.id WHEN MATCHED AND s793g.y > 0 THEN DELETE WHEN MATCHED THEN UPDATE SET x = s793g.x;",
  );
  assert!(!d.iter().any(|x| x.code == "sql793"));
}

#[test]
fn sql794_insert_references_target() {
  let d = diags(
    "MERGE INTO t794 USING s794 ON t794.id = s794.id WHEN NOT MATCHED THEN INSERT (id, val) VALUES (s794.id, t794.default_val);",
  );
  assert!(d.iter().any(|x| x.code == "sql794" && x.severity == Severity::Error));
}

#[test]
fn sql794_quiet_insert_references_only_source() {
  let d = diags(
    "MERGE INTO t794g USING s794g ON t794g.id = s794g.id WHEN NOT MATCHED THEN INSERT (id, val) VALUES (s794g.id, s794g.val);",
  );
  assert!(!d.iter().any(|x| x.code == "sql794"));
}
