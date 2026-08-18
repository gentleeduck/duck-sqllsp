# GROUPING SETS/CUBE/ROLLUP depth rules (batch 7) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan.

**Goal:** Add 3 new dsl-analysis lint rules (sql784-sql786) extending the existing `rollup_cube_single` with deeper GROUPING SETS/CUBE/ROLLUP checks.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 7). **Deviation:** original sql786 (`cube_rollup_empty_column_list`) verified unreachable -- `ROLLUP ()` / `CUBE ()` are hard pg_query parse rejections (`sql000: syntax error at or near ")"`), the same class of trap as batches 1, 2, and 4's swaps. Replacement, verified clean: **`rollup_cube_duplicate_column`** -- the same column listed twice inside a single ROLLUP/CUBE.

## Tasks

| Code | Rule | Severity | Detects |
| --- | --- | --- | --- |
| sql784 | `grouping_sets_duplicate_set` | Hint | Same grouping set (column-set, order-independent) appears twice in `GROUPING SETS (...)`. |
| sql785 | `grouping_function_arg_not_in_group_by` | Error | `GROUPING(x)` where `x` doesn't appear anywhere in the statement's `GROUP BY` clause text (PG 42803). Conservative: checks textual presence, not nested GROUPING SETS/ROLLUP/CUBE structure, so it never false-positives on a column that's actually grouped. |
| sql786 | `rollup_cube_duplicate_column` | Warning | Same column listed twice inside `ROLLUP (...)` or `CUBE (...)`. |

Files: `dsl-analysis/src/rules/{grouping_sets_duplicate_set,grouping_function_arg_not_in_group_by,rollup_cube_duplicate_column}.rs`, `mod.rs`, new `dsl-analysis/tests/rules_grouping_sets.rs`.

Tests (all verified parse-clean):

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
fn sql784_duplicate_set() {
  let d = diags("SELECT a, b, count(*) FROM t784 GROUP BY GROUPING SETS ((a,b), (a,b));");
  assert!(d.iter().any(|x| x.code == "sql784" && x.severity == Severity::Hint));
}

#[test]
fn sql784_quiet_distinct_sets() {
  let d = diags("SELECT a, b, count(*) FROM t784 GROUP BY GROUPING SETS ((a,b), (a));");
  assert!(!d.iter().any(|x| x.code == "sql784"));
}

#[test]
fn sql785_arg_not_in_group_by() {
  let d = diags("SELECT a, b, GROUPING(c) FROM t785 GROUP BY a, b;");
  assert!(d.iter().any(|x| x.code == "sql785" && x.severity == Severity::Error));
}

#[test]
fn sql785_quiet_when_arg_in_group_by() {
  let d = diags("SELECT a, b, GROUPING(a) FROM t785 GROUP BY a, b;");
  assert!(!d.iter().any(|x| x.code == "sql785"));
}

#[test]
fn sql786_duplicate_in_rollup() {
  let d = diags("SELECT a, count(*) FROM t786b GROUP BY ROLLUP (a, a);");
  assert!(d.iter().any(|x| x.code == "sql786" && x.severity == Severity::Warning));
}

#[test]
fn sql786_duplicate_in_cube() {
  let d = diags("SELECT a, count(*) FROM t786b GROUP BY CUBE (a, a);");
  assert!(d.iter().any(|x| x.code == "sql786" && x.severity == Severity::Warning));
}

#[test]
fn sql786_quiet_distinct_columns() {
  let d = diags("SELECT a, b, count(*) FROM t786b GROUP BY ROLLUP (a, b);");
  assert!(!d.iter().any(|x| x.code == "sql786"));
}
```

## Final step

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/grouping_sets_duplicate_set.rs \
        dsl-analysis/src/rules/grouping_function_arg_not_in_group_by.rs \
        dsl-analysis/src/rules/rollup_cube_duplicate_column.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_grouping_sets.rs \
        dsl-analysis/docs/plans/2026-08-18-grouping-sets-rules.md
git commit -m "feat(analysis): flag GROUPING SETS/CUBE/ROLLUP mistakes"
```
