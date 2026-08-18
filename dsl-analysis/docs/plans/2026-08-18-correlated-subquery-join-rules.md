# Correlated subquery / join depth rules (batch 8) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan.

**Goal:** Add 3 new dsl-analysis lint rules (sql787-sql789) covering correlated-scalar-subquery and forward/nullable-join-side footguns.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 8). No rule swap needed -- all 9 probe snippets verified parse-clean. One pre-existing quirk noted (not fixed): the existing `sql151` (`missing_lateral`) fires on both the sql788 bad and good probe snippets identically, suggesting it doesn't distinguish forward vs backward LATERAL references either -- out of scope for this batch, doesn't affect sql788's own correctness.

## Design notes

**sql787** deliberately narrows scope to avoid a real false-positive trap found during design: a subquery with its own internal `JOIN` (e.g. `(SELECT x.a FROM x JOIN y ON x.id=y.id)`) would make `y.id` look like an "outer" reference relative to `x` (the first FROM table) when it's actually the subquery's own second table. Fix: skip any subquery containing the word `JOIN` entirely -- only single-FROM-table subqueries are checked.

**sql788** uses `Scope` (not text-scanning for alias order) to find each `LATERAL`-referenced alias's *actual* source position (`Binding.table.range`), comparing it against the LATERAL clause's own position -- more robust than hand-parsing FROM/JOIN order.

**sql789** directly adapts the existing `left_join_defeated_by_where` (sql522): same conjunct-splitting, NULL-guard, and top-level-OR exemption logic, extended to collect aliases from *both* sides of `FULL [OUTER] JOIN` (both are nullable, unlike LEFT JOIN's one side).

## Tasks

| Code | Rule | Severity | Detects |
| --- | --- | --- | --- |
| sql787 | `correlated_subquery_select_no_limit1_no_agg` | Warning | `(SELECT ... FROM t WHERE t.x = outer.y)` -- correlated, single-FROM-table, no aggregate, no LIMIT. EXISTS/IN/ANY/ALL/SOME-wrapped subqueries are exempt. |
| sql788 | `lateral_join_references_later_table` | Error | `LATERAL (...)` references an alias whose `Scope` binding position is *after* the LATERAL clause -- forward reference, out of scope. |
| sql789 | `full_outer_join_where_defeats` | Warning | WHERE predicate on either side of `FULL [OUTER] JOIN` silently defeats it. Sibling to `left_join_defeated_by_where`. |

Files: `dsl-analysis/src/rules/{correlated_subquery_select_no_limit1_no_agg,lateral_join_references_later_table,full_outer_join_where_defeats}.rs`, `mod.rs`, new `dsl-analysis/tests/rules_correlated_join.rs`.

Tests:

```rust
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
```

## Final step

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/correlated_subquery_select_no_limit1_no_agg.rs \
        dsl-analysis/src/rules/lateral_join_references_later_table.rs \
        dsl-analysis/src/rules/full_outer_join_where_defeats.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_correlated_join.rs \
        dsl-analysis/docs/plans/2026-08-18-correlated-subquery-join-rules.md
git commit -m "feat(analysis): flag correlated subquery and join footguns"
```
