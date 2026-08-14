# Exclusion constraint rules (batch 4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan.

**Goal:** Add 3 new dsl-analysis lint rules (sql774-sql776) covering `EXCLUDE USING` constraint mistakes.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 4). **Deviation:** original sql774 (`exclude_using_no_operator`, missing operator after `WITH`) verified unreachable -- `EXCLUDE USING gist (during WITH))` is a hard pg_query parse rejection (`sql000: syntax error at or near ")"`), same class of trap as batches 1's and 2's swaps. Replacement, verified clean: **`exclude_using_duplicate_column`** -- the same column/expression listed twice in the exclusion list.

## Tasks

| Code | Rule | Severity | Detects |
| --- | --- | --- | --- |
| sql774 | `exclude_using_duplicate_column` | Warning | Same column/expr appears twice in `EXCLUDE USING <am> (...)`. |
| sql775 | `exclude_using_btree_index_type` | Error | `EXCLUDE USING {btree,hash,brin,gin}` -- only gist/spgist support exclusion constraints. |
| sql776 | `exclude_using_single_column_eq` | Hint | Single-column `EXCLUDE USING ... (col WITH =)` -- functionally a slower UNIQUE. |

Files: `dsl-analysis/src/rules/{exclude_using_duplicate_column,exclude_using_btree_index_type,exclude_using_single_column_eq}.rs`, `mod.rs`, new `dsl-analysis/tests/rules_exclusion.rs`.

Tests (all snippets verified parse-clean, no collisions):

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
fn sql774_duplicate_column() {
  let d = diags("CREATE TABLE t774b (id int, during tsrange, EXCLUDE USING gist (during WITH &&, during WITH &&));");
  assert!(d.iter().any(|x| x.code == "sql774" && x.severity == Severity::Warning));
}

#[test]
fn sql774_quiet_distinct_columns() {
  let d = diags("CREATE TABLE t774bg (id int, during tsrange, room int, EXCLUDE USING gist (room WITH =, during WITH &&));");
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
```

## Final step

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/exclude_using_duplicate_column.rs \
        dsl-analysis/src/rules/exclude_using_btree_index_type.rs \
        dsl-analysis/src/rules/exclude_using_single_column_eq.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_exclusion.rs \
        dsl-analysis/docs/plans/2026-08-18-exclusion-constraint-rules.md
git commit -m "feat(analysis): flag EXCLUDE constraint mistakes"
```
