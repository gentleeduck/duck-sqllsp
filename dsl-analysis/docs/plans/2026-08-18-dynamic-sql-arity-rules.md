# Dynamic SQL arity rules (batch 13) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan.

**Goal:** Add 2 new dsl-analysis lint rules (sql801-sql802) extending the existing `execute_string_concat`/`format_no_placeholders`/`raise_arg_count` family with EXECUTE/USING/INTO arity checks.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 13). No rule swap needed -- all 5 probe snippets verified to produce zero diagnostics.

**Design note:** sql801 scans the whole `EXECUTE <target>` text for `$N` placeholders regardless of whether the target is a plain string or wrapped in `format(...)` -- `format()`'s own `%s`/`%I`/`%L` substitutions are a completely separate mechanism from `USING`'s `$N` binding and are not counted.

## Tasks

| Code | Rule | Severity | Detects |
| --- | --- | --- | --- |
| sql801 | `execute_using_arg_count_mismatch` | Error | Highest `$N` placeholder in the EXECUTE target text doesn't match the `USING` argument count. |
| sql802 | `execute_into_arity_mismatch` | Warning | `EXECUTE '<literal SELECT items>' INTO a, b` where the statically-known column count doesn't match the INTO target count. Only fires when the target is a single plain string literal (not `format()`/concatenation) and the literal is a bare `SELECT <items> [FROM ...]`. |

Files: `dsl-analysis/src/rules/{execute_using_arg_count_mismatch,execute_into_arity_mismatch}.rs`, `mod.rs`, new `dsl-analysis/tests/rules_dynamic_sql_arity.rs`.

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
fn sql801_missing_using_arg() {
  let d = diags("DO $$ BEGIN EXECUTE 'SELECT * FROM t801 WHERE id = $1 AND x = $2' USING 5; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql801" && x.severity == Severity::Error));
}

#[test]
fn sql801_quiet_when_matching() {
  let d = diags("DO $$ BEGIN EXECUTE 'SELECT * FROM t801 WHERE id = $1 AND x = $2' USING 5, 'y'; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql801"));
}

#[test]
fn sql801_quiet_format_own_placeholders() {
  let d = diags("DO $$ BEGIN EXECUTE format('SELECT %I FROM t801 WHERE id = $1', 'col') USING 5; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql801"));
}

#[test]
fn sql802_arity_mismatch() {
  let d = diags("DO $$ DECLARE a int; b int; BEGIN EXECUTE 'SELECT 1, 2, 3' INTO a, b; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql802" && x.severity == Severity::Warning));
}

#[test]
fn sql802_quiet_when_matching() {
  let d = diags("DO $$ DECLARE a int; b int; BEGIN EXECUTE 'SELECT 1, 2' INTO a, b; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql802"));
}
```

## Final step

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/execute_using_arg_count_mismatch.rs \
        dsl-analysis/src/rules/execute_into_arity_mismatch.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_dynamic_sql_arity.rs \
        dsl-analysis/docs/plans/2026-08-18-dynamic-sql-arity-rules.md
git commit -m "feat(analysis): flag dynamic SQL EXECUTE/USING/INTO arity mismatches"
```
