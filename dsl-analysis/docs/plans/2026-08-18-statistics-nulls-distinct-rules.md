# Statistics object and NULLS NOT DISTINCT rules (batch 9) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan.

**Goal:** Add 3 new dsl-analysis lint rules (sql790-sql792) covering `UNIQUE NULLS NOT DISTINCT` redundancy and `CREATE STATISTICS` mistakes.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 9). No rule swap needed -- all 6 probe snippets verified parse-clean, including the single-column `CREATE STATISTICS ... (ndistinct) ON a FROM t` case (confirms it's a semantic rule, not dead code).

**Scoping note:** sql790 only checks the column-level inline form (`col type NOT NULL UNIQUE NULLS NOT DISTINCT`) -- a table-level `UNIQUE NULLS NOT DISTINCT (col)` constraint would need cross-referencing the column's separately-declared NOT NULL, out of scope for this first pass.

## Tasks

| Code | Rule | Severity | Detects |
| --- | --- | --- | --- |
| sql790 | `unique_nulls_distinct_redundant` | Hint | Column-level `NOT NULL ... UNIQUE NULLS NOT DISTINCT` on the same column -- NULLs can't occur, clause is a no-op. |
| sql791 | `create_statistics_no_columns` | Error | `CREATE STATISTICS name (ndistinct/dependencies) ON <1 column>` -- these kinds need 2+. |
| sql792 | `create_statistics_dup_column` | Warning | Same column named twice in `CREATE STATISTICS ... ON (a, a)`. |

Files: `dsl-analysis/src/rules/{unique_nulls_distinct_redundant,create_statistics_no_columns,create_statistics_dup_column}.rs`, `mod.rs`, new `dsl-analysis/tests/rules_statistics.rs`.

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
fn sql790_redundant_nulls_not_distinct() {
  let d = diags("CREATE TABLE t790 (id int, email text NOT NULL UNIQUE NULLS NOT DISTINCT);");
  assert!(d.iter().any(|x| x.code == "sql790" && x.severity == Severity::Hint));
}

#[test]
fn sql790_quiet_when_nullable() {
  let d = diags("CREATE TABLE t790g (id int, email text UNIQUE NULLS NOT DISTINCT);");
  assert!(!d.iter().any(|x| x.code == "sql790"));
}

#[test]
fn sql791_single_column() {
  let d = diags("CREATE STATISTICS s791 (ndistinct) ON a FROM t791;");
  assert!(d.iter().any(|x| x.code == "sql791" && x.severity == Severity::Error));
}

#[test]
fn sql791_quiet_two_columns() {
  let d = diags("CREATE STATISTICS s791g (ndistinct) ON a, b FROM t791;");
  assert!(!d.iter().any(|x| x.code == "sql791"));
}

#[test]
fn sql792_duplicate_column() {
  let d = diags("CREATE STATISTICS s792 (ndistinct) ON a, a FROM t792;");
  assert!(d.iter().any(|x| x.code == "sql792" && x.severity == Severity::Warning));
}

#[test]
fn sql792_quiet_distinct_columns() {
  let d = diags("CREATE STATISTICS s792g (ndistinct) ON a, b FROM t792;");
  assert!(!d.iter().any(|x| x.code == "sql792"));
}
```

## Final step

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/unique_nulls_distinct_redundant.rs \
        dsl-analysis/src/rules/create_statistics_no_columns.rs \
        dsl-analysis/src/rules/create_statistics_dup_column.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_statistics.rs \
        dsl-analysis/docs/plans/2026-08-18-statistics-nulls-distinct-rules.md
git commit -m "feat(analysis): flag statistics object and NULLS NOT DISTINCT mistakes"
```
