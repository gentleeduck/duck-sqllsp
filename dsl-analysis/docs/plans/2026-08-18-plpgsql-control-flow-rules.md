# PL/pgSQL control-flow rules (batch 12) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan.

**Goal:** Add 3 new dsl-analysis lint rules (sql798-sql800) extending the existing `exit_outside_loop`/`unreachable_after_return` with deeper PL/pgSQL loop/exception control-flow checks.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 12). No rule swap needed -- all 5 probe DO-block snippets verified to produce zero diagnostics of any kind.

**Design note:** sql799 checks the loop variable name against the whole connected `Catalog` (`catalog.columns_named`) rather than a per-statement `Scope` -- a PL/pgSQL function/DO body isn't resolved against a FROM-clause scope the way a bare SELECT is, so `Scope` wouldn't carry anything useful here.

## Tasks

| Code | Rule | Severity | Detects |
| --- | --- | --- | --- |
| sql798 | `loop_no_exit` | Warning | Bare `LOOP ... END LOOP` (not FOR/WHILE) whose body has no `EXIT`/`RETURN`/`RAISE` anywhere -- guaranteed infinite loop. Nested loops handled via LOOP/END LOOP depth tracking. |
| sql799 | `for_loop_variable_shadows_column` | Hint | `FOR i IN ...` where `i` matches a column name anywhere in the connected catalog. |
| sql800 | `exception_block_swallows_all` | Warning | `EXCEPTION WHEN OTHERS THEN` with an empty or `NULL;`-only body -- silently discards every error. |

Files: `dsl-analysis/src/rules/{loop_no_exit,for_loop_variable_shadows_column,exception_block_swallows_all}.rs`, `mod.rs`, new `dsl-analysis/tests/rules_plpgsql_control_flow.rs`.

Tests (sql799 needs a catalog fixture with a real column, unlike the other two):

```rust
use dsl_analysis::{Severity, run};
use dsl_catalog::{CATALOG_VERSION, Catalog, Column, Schema, Table, TableKind};
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

fn cat_with_id_column() -> Catalog {
  Catalog {
    version: CATALOG_VERSION,
    connection_id: "test".into(),
    schemas: vec![Schema {
      name: "public".into(),
      tables: vec![Table {
        schema: "public".into(),
        name: "t799".into(),
        kind: TableKind::Table,
        columns: vec![Column {
          name: "id".into(),
          data_type: "int".into(),
          nullable: false,
          default: None,
          comment: None,
          generated: None,
          json_keys: None,
        }],
        constraints: vec![],
        indexes: vec![],
        triggers: vec![],
        policies: vec![],
        comment: None,
        row_estimate: None,
        owner: None,
        definition: None,
        strict: false,
        options: None,
      }],
    }],
    functions: vec![],
    types: vec![],
    roles: vec![],
    sequences: vec![],
    extensions: vec![],
  }
}

fn diags(src: &str) -> Vec<dsl_analysis::Diagnostic> {
  diags_with_cat(src, &empty_cat())
}

fn diags_with_cat(src: &str, cat: &Catalog) -> Vec<dsl_analysis::Diagnostic> {
  let file = parse(src, Dialect::Postgres);
  let scopes = resolve_with_source(&file.statements, src);
  run(src, &file, &scopes, cat)
}

#[test]
fn sql798_bare_loop_no_exit() {
  let d = diags("DO $$ BEGIN LOOP PERFORM 1; END LOOP; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql798" && x.severity == Severity::Warning));
}

#[test]
fn sql798_quiet_with_exit() {
  let d = diags("DO $$ BEGIN LOOP EXIT WHEN true; END LOOP; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql798"));
}

#[test]
fn sql799_loop_var_shadows_column() {
  let d = diags_with_cat(
    "DO $$ BEGIN FOR id IN SELECT generate_series(1,3) LOOP RAISE NOTICE '%', id; END LOOP; END; $$;",
    &cat_with_id_column(),
  );
  assert!(d.iter().any(|x| x.code == "sql799" && x.severity == Severity::Hint));
}

#[test]
fn sql799_quiet_without_catalog() {
  let d = diags("DO $$ BEGIN FOR id IN SELECT generate_series(1,3) LOOP RAISE NOTICE '%', id; END LOOP; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql799"));
}

#[test]
fn sql800_null_only_handler() {
  let d = diags("DO $$ BEGIN RAISE EXCEPTION 'test'; EXCEPTION WHEN OTHERS THEN NULL; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql800" && x.severity == Severity::Warning));
}

#[test]
fn sql800_quiet_with_real_handling() {
  let d = diags("DO $$ BEGIN RAISE EXCEPTION 'test'; EXCEPTION WHEN OTHERS THEN RAISE NOTICE 'caught: %', SQLERRM; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql800"));
}
```

## Final step

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/loop_no_exit.rs \
        dsl-analysis/src/rules/for_loop_variable_shadows_column.rs \
        dsl-analysis/src/rules/exception_block_swallows_all.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_plpgsql_control_flow.rs \
        dsl-analysis/docs/plans/2026-08-18-plpgsql-control-flow-rules.md
git commit -m "feat(analysis): flag PL/pgSQL loop and exception control-flow mistakes"
```
