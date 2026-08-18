# Recursive CTE restriction rules (batch 3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Process/global constraints match batch 1's plan exactly.

**Goal:** Add 5 new dsl-analysis lint rules (sql769-sql773) encoding PostgreSQL's hard restrictions on a `WITH RECURSIVE` query's recursive term.

**Architecture:** All 5 rules share a new helper, `clause_scan::find_recursive_cte`, added to the existing shared clause-scanning module (not duplicated 5x -- this is genuinely reused domain logic, unlike the tiny match_paren/skip_ws helpers that stay per-file per convention). It finds the byte span of the recursive term in a single-CTE `WITH RECURSIVE name[(cols)] AS (base UNION [ALL] recursive)` query. Multi-CTE `WITH RECURSIVE a AS (...), b AS (...)` lists are out of scope (`None`) -- kept conservative.

**Spec:** `dsl-analysis/docs/specs/2026-08-18-sql-rule-expansion-design.md` (batch 3 section). No deviations this batch -- all 5 rules verified to parse cleanly and not collide with existing rules (probe results below), unlike batches 1 and 2 which each needed one swap.

**Verification performed before writing any Rust** (`duck-sqllsp lint` against 9 snippets covering all 5 bad cases + relevant good cases): zero `sql000` parse errors, zero unexpected collisions with the existing 666 rules. One unrelated finding: `sql017` misfires on a `UNION ALL`-containing recursive CTE body (attributes an aggregate-without-GROUP-BY warning to the wrong branch) -- pre-existing bug in an existing rule, out of scope, not fixed here.

## Task 0: shared helper

**Files:** Modify `dsl-analysis/src/clause_scan.rs` (append only, no changes to existing exports).

Add `pub struct RecursiveCte { name_start, name_end, cols: Vec<String>, term_start, term_end }`, `pub fn find_recursive_cte(body: &str, upper: &str) -> Option<RecursiveCte>`, and `pub fn unwrap_parens(ub: &[u8], start: usize, end: usize) -> (usize, usize)` (strips wrapping parens for depth-0 scans, e.g. when the recursive term is written as `UNION ALL (SELECT ... ORDER BY ...)`). Full code already applied directly (see the file) -- this task has no separate test of its own; it's exercised transitively by tasks 1-5's tests.

## Tasks 1-5: sql769-sql773

Each rule: create `dsl-analysis/src/rules/<name>.rs`, register in `mod.rs`, add tests to new `dsl-analysis/tests/rules_recursive_cte.rs`. All Error severity (each encodes a hard PG rejection, not a style smell).

| Code | Rule | Detects |
| --- | --- | --- |
| sql769 | `recursive_cte_cycle_column_reused` | `CYCLE ... USING <col>` where `<col>` collides with an existing CTE column name. `SEARCH ... SET <col>` collision is out of scope for this first pass (documented in the rule's doc comment). |
| sql770 | `recursive_cte_missing_base_union` | The recursive term references the CTE name 2+ times (PG allows exactly one self-reference). |
| sql771 | `recursive_term_has_aggregate` | An aggregate function call inside the recursive term. |
| sql772 | `recursive_term_has_order_or_limit` | Top-level `ORDER BY`/`LIMIT`/`DISTINCT` inside the recursive term (nested subqueries are exempt via `unwrap_parens` + depth-0 `find_clause`). |
| sql773 | `recursive_cte_outer_join_recursive_side` | Self-reference on the nullable side of `LEFT JOIN`/`RIGHT JOIN`/`FULL [OUTER] JOIN` inside the recursive term. |

Test fixtures (verified parse-clean above):

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
  let d = diags("WITH RECURSIVE t770(id) AS (SELECT 1 UNION ALL SELECT a.id FROM t770 a JOIN t770 b ON a.id = b.id + 1) SELECT * FROM t770;");
  assert!(d.iter().any(|x| x.code == "sql770" && x.severity == Severity::Error));
}

#[test]
fn sql770_quiet_single_self_reference() {
  let d = diags("WITH RECURSIVE t770g(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM t770g WHERE id < 10) SELECT * FROM t770g;");
  assert!(!d.iter().any(|x| x.code == "sql770"));
}

#[test]
fn sql771_aggregate_in_recursive_term() {
  let d = diags("WITH RECURSIVE t771(id) AS (SELECT 1 UNION ALL SELECT count(*) FROM t771) SELECT * FROM t771;");
  assert!(d.iter().any(|x| x.code == "sql771" && x.severity == Severity::Error));
}

#[test]
fn sql771_quiet_no_aggregate() {
  let d = diags("WITH RECURSIVE t771g(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM t771g WHERE id < 10) SELECT * FROM t771g;");
  assert!(!d.iter().any(|x| x.code == "sql771"));
}

#[test]
fn sql772_order_by_limit_in_recursive_term() {
  let d = diags("WITH RECURSIVE t772(id) AS (SELECT 1 UNION ALL (SELECT id + 1 FROM t772 ORDER BY id LIMIT 5)) SELECT * FROM t772;");
  assert!(d.iter().any(|x| x.code == "sql772" && x.severity == Severity::Error));
}

#[test]
fn sql772_quiet_without_order_or_limit() {
  let d = diags("WITH RECURSIVE t772g(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM t772g WHERE id < 10) SELECT * FROM t772g;");
  assert!(!d.iter().any(|x| x.code == "sql772"));
}

#[test]
fn sql773_self_reference_on_left_join_nullable_side() {
  let d = diags("WITH RECURSIVE t773(id) AS (SELECT 1 UNION ALL SELECT n.id FROM nodes n LEFT JOIN t773 ON n.parent_id = t773.id) SELECT * FROM t773;");
  assert!(d.iter().any(|x| x.code == "sql773" && x.severity == Severity::Error));
}

#[test]
fn sql773_quiet_on_inner_join() {
  let d = diags("WITH RECURSIVE t773g(id) AS (SELECT 1 UNION ALL SELECT n.id FROM nodes n JOIN t773g ON n.parent_id = t773g.id) SELECT * FROM t773g;");
  assert!(!d.iter().any(|x| x.code == "sql773"));
}
```

## Final step: verify and commit

```bash
cargo test --workspace --release
cargo clippy -p dsl-analysis --all-features --release -- -D warnings
git add dsl-analysis/src/clause_scan.rs \
        dsl-analysis/src/rules/recursive_cte_cycle_column_reused.rs \
        dsl-analysis/src/rules/recursive_cte_missing_base_union.rs \
        dsl-analysis/src/rules/recursive_term_has_aggregate.rs \
        dsl-analysis/src/rules/recursive_term_has_order_or_limit.rs \
        dsl-analysis/src/rules/recursive_cte_outer_join_recursive_side.rs \
        dsl-analysis/src/rules/mod.rs \
        dsl-analysis/tests/rules_recursive_cte.rs \
        dsl-analysis/docs/plans/2026-08-18-recursive-cte-rules.md
git commit -m "feat(analysis): flag recursive CTE restriction violations"
```
