# Logical replication rules (batch 11) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan.

**Goal:** Add 3 new dsl-analysis lint rules (sql795-sql797) covering `CREATE PUBLICATION`/`CREATE SUBSCRIPTION` mistakes.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 11). **Deviation:** original sql795 (`publication_for_all_tables_and_list`, `FOR ALL TABLES, TABLE x`) verified unreachable -- `FOR ALL TABLES` and `FOR TABLE` are mutually exclusive grammar productions; pg_query rejects the comma-combination as a hard parse error (`sql000: syntax error at or near ","`), the 5th occurrence of this exact trap across the plan (batches 1, 2, 4, 7, 11). Replacement, verified clean: **`publication_duplicate_schema`** -- the same schema listed twice in `FOR TABLES IN SCHEMA`.

## Tasks

| Code | Rule | Severity | Detects |
| --- | --- | --- | --- |
| sql795 | `publication_duplicate_schema` | Error | Same schema listed twice in `CREATE PUBLICATION ... FOR TABLES IN SCHEMA s, s`. |
| sql796 | `subscription_no_slot_name_with_create_false` | Error | `CREATE SUBSCRIPTION ... WITH (create_slot = false)` with no `slot_name` -- PG can't infer which slot to use. |
| sql797 | `publication_duplicate_table` | Error | Same table listed twice in `CREATE PUBLICATION ... FOR TABLE a, a`. |

Files: `dsl-analysis/src/rules/{publication_duplicate_schema,subscription_no_slot_name_with_create_false,publication_duplicate_table}.rs`, `mod.rs`, new `dsl-analysis/tests/rules_replication.rs`.

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
```

## Final step

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/publication_duplicate_schema.rs \
        dsl-analysis/src/rules/subscription_no_slot_name_with_create_false.rs \
        dsl-analysis/src/rules/publication_duplicate_table.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_replication.rs \
        dsl-analysis/docs/plans/2026-08-18-logical-replication-rules.md
git commit -m "feat(analysis): flag logical replication mistakes"
```
