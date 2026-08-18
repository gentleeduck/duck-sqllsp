# Domain and composite type rules (batch 5) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan.

**Goal:** Add 3 new dsl-analysis lint rules (sql777-sql779) covering `CREATE DOMAIN` and `CREATE TYPE ... AS (...)` mistakes.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 5). No deviations -- all 6 probe snippets (3 bad + 3 good) verified to produce zero diagnostics of any kind (not even unrelated noise), confirming clean reachability and zero collision.

## Tasks

| Code | Rule | Severity | Detects |
| --- | --- | --- | --- |
| sql777 | `domain_check_references_value_missing` | Warning | `CREATE DOMAIN ... CHECK (expr)` where `expr` never references `VALUE`. |
| sql778 | `domain_default_violates_check` | Warning | `DEFAULT <literal>` that fails an adjacent `CHECK (VALUE <op> <literal>)` (both sides literal-evaluable). Warning, not Error -- PG's exact validation timing for domain defaults isn't confirmed. |
| sql779 | `composite_type_dup_field` | Error | `CREATE TYPE ... AS (a int, a text)` -- duplicate field name. |

Files: `dsl-analysis/src/rules/{domain_check_references_value_missing,domain_default_violates_check,composite_type_dup_field}.rs`, `mod.rs`, new `dsl-analysis/tests/rules_domain_composite.rs`.

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
fn sql777_check_missing_value() {
  let d = diags("CREATE DOMAIN age777 AS int CHECK (1 > 0);");
  assert!(d.iter().any(|x| x.code == "sql777" && x.severity == Severity::Warning));
}

#[test]
fn sql777_quiet_when_value_referenced() {
  let d = diags("CREATE DOMAIN age777g AS int CHECK (VALUE > 0);");
  assert!(!d.iter().any(|x| x.code == "sql777"));
}

#[test]
fn sql778_default_violates_check() {
  let d = diags("CREATE DOMAIN age778 AS int CHECK (VALUE > 0) DEFAULT -1;");
  assert!(d.iter().any(|x| x.code == "sql778" && x.severity == Severity::Warning));
}

#[test]
fn sql778_quiet_when_default_satisfies_check() {
  let d = diags("CREATE DOMAIN age778g AS int CHECK (VALUE > 0) DEFAULT 5;");
  assert!(!d.iter().any(|x| x.code == "sql778"));
}

#[test]
fn sql779_duplicate_field() {
  let d = diags("CREATE TYPE t779 AS (a int, a text);");
  assert!(d.iter().any(|x| x.code == "sql779" && x.severity == Severity::Error));
}

#[test]
fn sql779_quiet_distinct_fields() {
  let d = diags("CREATE TYPE t779g AS (a int, b text);");
  assert!(!d.iter().any(|x| x.code == "sql779"));
}
```

## Final step

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/domain_check_references_value_missing.rs \
        dsl-analysis/src/rules/domain_default_violates_check.rs \
        dsl-analysis/src/rules/composite_type_dup_field.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_domain_composite.rs \
        dsl-analysis/docs/plans/2026-08-18-domain-composite-rules.md
git commit -m "feat(analysis): flag domain and composite type mistakes"
```
