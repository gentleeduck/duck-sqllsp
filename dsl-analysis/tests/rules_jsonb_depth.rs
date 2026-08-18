//! Batch 6 of the SQL rule expansion plan (sql780-sql783): jsonpath
//! and jsonb-literal operator depth. See
//! dsl-analysis/docs/plans/2026-08-18-jsonpath-jsonb-depth-rules.md.

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
fn sql780_static_false_filter() {
  let d = diags("SELECT jsonb_path_exists(doc, '$.a ? (1 == 2)') FROM t780;");
  assert!(d.iter().any(|x| x.code == "sql780" && x.severity == Severity::Warning));
}

#[test]
fn sql780_quiet_on_dynamic_filter() {
  let d = diags("SELECT jsonb_path_exists(doc, '$.a ? (@ == 2)') FROM t780;");
  assert!(!d.iter().any(|x| x.code == "sql780"));
}

#[test]
fn sql781_array_length_on_object() {
  let d = diags(r#"SELECT jsonb_array_length('{"a":1}'::jsonb);"#);
  assert!(d.iter().any(|x| x.code == "sql781" && x.severity == Severity::Error));
}

#[test]
fn sql781_quiet_on_array_literal() {
  let d = diags("SELECT jsonb_array_length('[1,2,3]'::jsonb);");
  assert!(!d.iter().any(|x| x.code == "sql781"));
}

#[test]
fn sql782_minus_integer_on_object_literal() {
  let d = diags(r#"SELECT '{"a":1}'::jsonb - 0;"#);
  assert!(d.iter().any(|x| x.code == "sql782" && x.severity == Severity::Error));
}

#[test]
fn sql782_quiet_on_array_literal() {
  let d = diags("SELECT '[1,2,3]'::jsonb - 0;");
  assert!(!d.iter().any(|x| x.code == "sql782"));
}

#[test]
fn sql783_null_key() {
  let d = diags("SELECT jsonb_build_object(NULL, 1);");
  assert!(d.iter().any(|x| x.code == "sql783" && x.severity == Severity::Error));
}

#[test]
fn sql783_quiet_string_key() {
  let d = diags("SELECT jsonb_build_object('k', 1);");
  assert!(!d.iter().any(|x| x.code == "sql783"));
}
