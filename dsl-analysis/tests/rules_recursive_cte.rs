//! Batch 3 of the SQL rule expansion plan (sql769-sql773): recursive
//! CTE hard restrictions. See
//! dsl-analysis/docs/plans/2026-08-18-recursive-cte-rules.md.

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
fn sql769_cycle_using_collides_with_cte_column() {
  let d = diags(
    "WITH RECURSIVE t769(id, path) AS (SELECT id, id::text FROM nodes WHERE parent_id IS NULL UNION ALL SELECT n.id, t769.path || '.' || n.id FROM nodes n JOIN t769 ON n.parent_id = t769.id) CYCLE id SET is_cycle USING path SELECT * FROM t769;",
  );
  assert!(d.iter().any(|x| x.code == "sql769" && x.severity == Severity::Error));
}

#[test]
fn sql769_quiet_when_using_col_is_new() {
  let d = diags(
    "WITH RECURSIVE t769g(id, path) AS (SELECT id, id::text FROM nodes WHERE parent_id IS NULL UNION ALL SELECT n.id, t769g.path || '.' || n.id FROM nodes n JOIN t769g ON n.parent_id = t769g.id) CYCLE id SET is_cycle USING cyc_path SELECT * FROM t769g;",
  );
  assert!(!d.iter().any(|x| x.code == "sql769"));
}

#[test]
fn sql770_multiple_self_reference() {
  let d = diags(
    "WITH RECURSIVE t770(id) AS (SELECT 1 UNION ALL SELECT a.id FROM t770 a JOIN t770 b ON a.id = b.id + 1) SELECT * FROM t770;",
  );
  assert!(d.iter().any(|x| x.code == "sql770" && x.severity == Severity::Error));
}

#[test]
fn sql770_quiet_single_self_reference() {
  let d = diags(
    "WITH RECURSIVE t770g(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM t770g WHERE id < 10) SELECT * FROM t770g;",
  );
  assert!(!d.iter().any(|x| x.code == "sql770"));
}

#[test]
fn sql771_aggregate_in_recursive_term() {
  let d = diags("WITH RECURSIVE t771(id) AS (SELECT 1 UNION ALL SELECT count(*) FROM t771) SELECT * FROM t771;");
  assert!(d.iter().any(|x| x.code == "sql771" && x.severity == Severity::Error));
}

#[test]
fn sql771_quiet_no_aggregate() {
  let d = diags(
    "WITH RECURSIVE t771g(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM t771g WHERE id < 10) SELECT * FROM t771g;",
  );
  assert!(!d.iter().any(|x| x.code == "sql771"));
}

#[test]
fn sql772_order_by_limit_in_recursive_term() {
  let d = diags(
    "WITH RECURSIVE t772(id) AS (SELECT 1 UNION ALL (SELECT id + 1 FROM t772 ORDER BY id LIMIT 5)) SELECT * FROM t772;",
  );
  assert!(d.iter().any(|x| x.code == "sql772" && x.severity == Severity::Error));
}

#[test]
fn sql772_quiet_without_order_or_limit() {
  let d = diags(
    "WITH RECURSIVE t772g(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM t772g WHERE id < 10) SELECT * FROM t772g;",
  );
  assert!(!d.iter().any(|x| x.code == "sql772"));
}

#[test]
fn sql773_self_reference_on_left_join_nullable_side() {
  let d = diags(
    "WITH RECURSIVE t773(id) AS (SELECT 1 UNION ALL SELECT n.id FROM nodes n LEFT JOIN t773 ON n.parent_id = t773.id) SELECT * FROM t773;",
  );
  assert!(d.iter().any(|x| x.code == "sql773" && x.severity == Severity::Error));
}

#[test]
fn sql773_quiet_on_inner_join() {
  let d = diags(
    "WITH RECURSIVE t773g(id) AS (SELECT 1 UNION ALL SELECT n.id FROM nodes n JOIN t773g ON n.parent_id = t773g.id) SELECT * FROM t773g;",
  );
  assert!(!d.iter().any(|x| x.code == "sql773"));
}
