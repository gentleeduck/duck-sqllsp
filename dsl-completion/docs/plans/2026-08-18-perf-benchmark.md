# Completion perf benchmark (Phase A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline, no subagents). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a performance smoke-timing test for `dsl-completion`, establishing the "before" baseline for the redesign's Phase D re-measurement.

**Architecture:** Mirror `dsl-analysis/tests/perf_bench.rs` exactly -- a single `#[ignore]`'d test using `std::time::Instant` and `eprintln!`, not a statistical percentile harness. The spec's "p50/p95" phrasing was aspirational; the actual codebase convention (confirmed by reading the existing file) is a single aggregate timing run over many calls, reported as an average. This plan follows the real convention, not the spec's wording, since the spec's intent was "match dsl-analysis's existing pattern" and the pattern turned out to be simpler than assumed.

**Tech Stack:** Rust, `dsl-completion` crate, `dsl_parse`/`dsl_resolve`/`dsl_catalog` (same dependencies `dsl-analysis`'s bench already uses).

**Spec:** `dsl-completion/docs/specs/2026-08-18-completion-engine-redesign-design.md` (Phase A).

## Global Constraints

- Test must be `#[ignore]`d (like `dsl-analysis`'s) so it doesn't run in the default `cargo test` pass -- it's a manual/CI-opt-in smoke check, not a correctness assertion.
- No changes to `dsl-completion/src/*` in this task -- purely additive test file.
- `cargo test -p dsl-completion --release -- --ignored --nocapture` must run the new test and print timing output.

---

## Task 1: Add `perf_bench.rs`

**Files:**
- Create: `dsl-completion/tests/perf_bench.rs`

**Interfaces:**
- Consumes: `dsl_completion::complete(source: &str, file: &ParsedFile, scopes: &[Scope], catalog: &Catalog, offset: TextSize) -> Vec<Item>` (existing public API, re-exported from `dsl-completion/src/lib.rs` as `pub use engine::complete`).
- Produces: nothing consumed elsewhere -- a standalone `#[ignore]`d test.

- [ ] **Step 1: Write the benchmark test**

```rust
use dsl_catalog::Catalog;
use dsl_completion::complete;
use dsl_parse::{Dialect, parse};
use dsl_resolve::resolve_with_source;
use text_size::TextSize;

#[test]
#[ignore]
fn perf_10k_stmts_complete_after_each() {
  let mut s = String::with_capacity(500_000);
  let mut offsets = Vec::with_capacity(10_000);
  for i in 0..10_000 {
    s.push_str(&format!("SELECT id FROM users WHERE id = {i}"));
    offsets.push(TextSize::from(s.len() as u32));
    s.push_str(";\n");
  }
  let t0 = std::time::Instant::now();
  let file = parse(&s, Dialect::Postgres);
  let p = t0.elapsed();
  let scopes = resolve_with_source(&file.statements, &s);
  let r = t0.elapsed();
  let cat = Catalog::default();
  let c0 = std::time::Instant::now();
  for &off in &offsets {
    let _ = complete(&s, &file, &scopes, &cat, off);
  }
  let complete_elapsed = c0.elapsed();
  let elapsed = t0.elapsed();
  eprintln!(
    "parse: {:?}  resolve: {:?}  complete(10k calls): {:?}  avg/call: {:?}  total: {:?}",
    p,
    r - p,
    complete_elapsed,
    complete_elapsed / 10_000,
    elapsed
  );
}
```

- [ ] **Step 2: Verify it compiles and runs**

Run: `cargo test -p dsl-completion --release --test perf_bench -- --ignored --nocapture`
Expected: compiles clean, prints a line like `parse: ...  resolve: ...  complete(10k calls): ...  avg/call: ...  total: ...` with no panics. Record the printed numbers in the commit message as the Phase A baseline.

- [ ] **Step 3: Run full test suite to confirm nothing else broke**

Run: `cargo test -p dsl-completion --release 2>&1 | tail -40`
Expected: all existing tests still pass (this task is purely additive).

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p dsl-completion --all-features --release -- -D warnings`
Expected: clean. If not, fix inline (small file, should be trivial).

- [ ] **Step 5: Commit**

```bash
git add dsl-completion/tests/perf_bench.rs dsl-completion/docs/plans/2026-08-18-perf-benchmark.md
git commit -m "test(completion): add perf smoke-timing benchmark

Phase A of the completion engine redesign -- establishes the baseline
complete() timing before the Phase B architectural refactor. Mirrors
dsl-analysis/tests/perf_bench.rs's existing convention (single
#[ignore]'d test, Instant-based timing, eprintln report) rather than
a statistical percentile harness."
```
