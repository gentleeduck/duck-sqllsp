# Perf and dead-code smell rules (batch 14, final) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan.

**Goal:** Add the final 2 new dsl-analysis lint rules (sql803-sql804), completing the 48-rule SQL rule expansion plan.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 14). No rule swap needed -- all 4 probe snippets verified to produce zero diagnostics.

**Scoping notes:** sql803 flags any `RAISE NOTICE` inside a loop body regardless of further conditional nesting inside that loop -- precise "is this actually unconditional" control-flow analysis is out of scope; it's a nudge, not a certainty. sql804 only handles the simple single top-level `DECLARE ... BEGIN` block shape.

## Tasks

| Code | Rule | Severity | Detects |
| --- | --- | --- | --- |
| sql803 | `raise_notice_in_hot_loop` | Hint | `RAISE NOTICE` anywhere inside a loop body (bare/FOR/WHILE, matched via the same LOOP/END LOOP depth tracking as sql798). |
| sql804 | `variable_declared_unused` | Hint | A `DECLARE x type;` variable never referenced anywhere after `BEGIN`. |

Files: `dsl-analysis/src/rules/{raise_notice_in_hot_loop,variable_declared_unused}.rs`, `mod.rs`, new `dsl-analysis/tests/rules_perf_deadcode.rs`.

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
fn sql803_raise_notice_in_loop() {
  let d = diags("DO $$ BEGIN FOR i IN 1..10 LOOP RAISE NOTICE 'processing %', i; END LOOP; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql803" && x.severity == Severity::Hint));
}

#[test]
fn sql803_quiet_without_raise_notice() {
  let d = diags("DO $$ BEGIN FOR i IN 1..10 LOOP PERFORM i; END LOOP; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql803"));
}

#[test]
fn sql804_unused_variable() {
  let d = diags("DO $$ DECLARE unused_var804 int; BEGIN RAISE NOTICE 'hello'; END; $$;");
  assert!(d.iter().any(|x| x.code == "sql804" && x.severity == Severity::Hint));
}

#[test]
fn sql804_quiet_when_used() {
  let d = diags("DO $$ DECLARE used_var804 int; BEGIN used_var804 := 5; RAISE NOTICE '%', used_var804; END; $$;");
  assert!(!d.iter().any(|x| x.code == "sql804"));
}
```

## Final step (also closes out the whole 14-batch plan)

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/rules/raise_notice_in_hot_loop.rs \
        dsl-analysis/src/rules/variable_declared_unused.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_perf_deadcode.rs \
        dsl-analysis/docs/plans/2026-08-18-perf-deadcode-rules.md
git commit -m "feat(analysis): flag loop-body RAISE NOTICE and unused PL/pgSQL variables"
```

After this commit, all 48 planned rules (sql757-sql804) are implemented across 14 batches. See the spec doc for the full inventory and the per-batch plan docs under `dsl-analysis/docs/plans/` for implementation detail and the (5) rule swaps made along the way.
