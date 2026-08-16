//! Batch 8 of the SQL rule expansion plan (sql787-sql789): correlated
//! subquery and join footguns. See
//! dsl-analysis/docs/plans/2026-08-18-correlated-subquery-join-rules.md.

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
fn sql787_correlated_no_agg_no_limit() {
  let d = diags("SELECT o.id, (SELECT p.name FROM products787 p WHERE p.id = o.product_id) FROM orders787 o;");
  assert!(d.iter().any(|x| x.code == "sql787" && x.severity == Severity::Warning));
}

#[test]
fn sql787_quiet_with_limit() {
  let d = diags("SELECT o.id, (SELECT p.name FROM products787 p WHERE p.id = o.product_id LIMIT 1) FROM orders787 o;");
  assert!(!d.iter().any(|x| x.code == "sql787"));
}

#[test]
fn sql787_quiet_with_aggregate() {
  let d = diags("SELECT o.id, (SELECT count(*) FROM products787 p WHERE p.id = o.product_id) FROM orders787 o;");
  assert!(!d.iter().any(|x| x.code == "sql787"));
}

#[test]
fn sql787_quiet_on_exists() {
  let d = diags("SELECT o.id FROM orders787 o WHERE EXISTS (SELECT 1 FROM products787 p WHERE p.id = o.product_id);");
  assert!(!d.iter().any(|x| x.code == "sql787"));
}

#[test]
fn sql788_forward_reference() {
  let d = diags("SELECT * FROM a788, LATERAL (SELECT * FROM b788 WHERE b788.x = c788.y) sub, c788;");
  assert!(d.iter().any(|x| x.code == "sql788" && x.severity == Severity::Error));
}

#[test]
fn sql788_quiet_backward_reference() {
  let d = diags("SELECT * FROM a788, c788, LATERAL (SELECT * FROM b788 WHERE b788.x = c788.y) sub;");
  assert!(!d.iter().any(|x| x.code == "sql788"));
}

#[test]
fn sql789_where_defeats_full_join() {
  let d = diags("SELECT * FROM a789 FULL JOIN b789 ON a789.id = b789.id WHERE b789.status = 'active';");
  assert!(d.iter().any(|x| x.code == "sql789" && x.severity == Severity::Warning));
}

#[test]
fn sql789_quiet_without_filter() {
  let d = diags("SELECT * FROM a789 FULL JOIN b789 ON a789.id = b789.id;");
  assert!(!d.iter().any(|x| x.code == "sql789"));
}
