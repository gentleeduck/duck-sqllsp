# Completion engine redesign: design

Status: approved, pending implementation.
Owner: dsl-completion crate.
Date: 2026-08-18.

## Motivation

`dsl-completion` provides context-aware SQL completion across ~55
`Phase` states (`dsl-completion/src/phase.rs`). The engine that drives
it, `dsl-completion/src/engine.rs`, has grown to **10,925 lines and 210
top-level functions in one file**, with **zero section-comment
structure** (contrast `phase.rs`, which uses `// ----- DML -----` style
dividers throughout). Its test file, `dsl-completion/tests/engine.rs`,
is **19,243 lines**.

The architecture inside that file is the deeper problem, not just its
size. Completion resolution runs through three separate, informally
ordered "short-circuit" layers:

1. `complete()` (the public entry point, ~300 lines): roughly ten
   pattern checks, each returning early if it matches, before the
   engine even computes a `Phase`.
2. `contexts::detect()` (`dsl-completion/src/contexts.rs`, 447 lines):
   its own doc comment describes it as catching "situations the phase
   state machine doesn't model" -- a second, independent short-circuit
   layer, called separately from `complete()`.
3. `route_phase()` (~1,576 lines): *more* short-circuit checks, then
   finally a `match` over all 55 `Phase` variants, several of whose
   arms contain substantial inline logic.

Every one of these short-circuits carries a hand-written comment
explaining why it must run before some other check -- e.g. "must beat
every phase variant", "must beat the SELECT trailing-clause menu",
"Only short-circuit on the frame-bound sub-chain". That reasoning is
real and was clearly hard-won, but it lives as prose scattered across
2,000+ lines of control flow instead of as a single place you can read
to answer "in what order do these run, and why". Adding a new
completion case means finding the right spot in that implicit ordering
-- exactly the kind of thing that gets harder, not easier, as the file
grows.

There is no dedicated performance benchmark for this crate (`dsl-
analysis` has `tests/perf_bench.rs`; `dsl-completion` does not), even
though the README states a `Completion p50 < 5 ms` target.

## Evidence (reproducible)

```sh
wc -l dsl-completion/src/*.rs dsl-completion/tests/*.rs | sort -n
# engine.rs: 10925 lines; tests/engine.rs: 19243 lines
grep -c "^pub fn \|^fn " dsl-completion/src/engine.rs   # 210
grep -c "Phase::" dsl-completion/src/engine.rs           # 48
grep -n "^  // -----\|^// -----" dsl-completion/src/engine.rs  # (no matches)
grep -oE "^  [A-Za-z]+,$|^  [A-Za-z]+ \{" dsl-completion/src/phase.rs | wc -l  # 55 Phase variants
```

Keyword-presence sweep (advanced SQL surface, same themes as the
recent `dsl-analysis` rule-expansion project) across `dsl-completion/
src/*.rs`: MERGE (39 hits), GROUPING SETS (15), PARTITION BY (22),
ATTACH PARTITION (8), EXCLUDE USING (5), CREATE STATISTICS (4), CREATE
PUBLICATION (5), CREATE SUBSCRIPTION (5), IS JSON (6), NULLS NOT
DISTINCT (3), LATERAL (15), **JSON_TABLE (0)**.

This is a materially different coverage picture than the lint-rule
work: most "advanced" constructs already have *some* completion
awareness. The gaps here are more likely to be precision gaps (offers
the wrong thing, or nothing, at a specific cursor position within an
otherwise-handled construct) than whole-construct blindness. `JSON_
TABLE` is the one clean exception -- confirmed zero presence.

## Scope decisions (from brainstorming)

- Coverage focus: all four raised -- newer/advanced SQL surface,
  deeper awareness within existing phases, cross-file/workspace
  intelligence, and (explicitly) using judgment to run a real gap
  analysis rather than assuming a fixed list up front.
- Redesign approach: **incremental refactor, tests green at every
  step** -- not a clean-slate rewrite. The 19,243-line test suite
  encodes a large amount of validated behavior; it's the safety net
  for this work, not a burden to discard.
- Performance: **establish a benchmark first**, mirroring `dsl-
  analysis`'s existing `perf_bench.rs` pattern, before making any
  claim about "faster".

## Plan: four phases

### Phase A -- Performance safety net

Add `dsl-completion/tests/perf_bench.rs`. Measures `complete()` p50/
p95 over a representative corpus of (source, cursor offset) pairs
spanning multiple phases -- mirrors whatever pattern `dsl-analysis/
tests/perf_bench.rs` already establishes for this workspace. This
produces the *before* numbers; Phase D re-runs it for *after*. No
behavior change, so this phase is low-risk and worth doing first and
in isolation.

### Phase B -- Incremental architectural refactor

The target shape: replace the three informal short-circuit layers with
one explicit, ordered **detector registry**.

- A detector is a named, independently testable function matching a
  common shape: given the current `(source, offset, file, scopes,
  catalog)`, return `Option<Vec<Item>>` -- `Some` short-circuits,
  `None` falls through to the next detector.
- A single ordered list (e.g. `const DETECTORS: &[Detector]` in
  `engine.rs`, or a `Vec` built once) is the *only* place priority
  order lives. Each entry keeps the "why this must come before/after
  X" comment that already exists today -- just relocated from
  scattered control flow into one scannable list.
- `contexts::detect()`'s cases fold into this same registry instead of
  being a separate parallel mechanism.
- The final fallback after every detector returns `None` is the
  existing `Phase`-driven `match` -- but trimmed down to just the
  "ordinary, no-special-case" behavior per phase, since the special
  cases will have moved out into named detectors.
- Module split: group detectors by statement family into files under
  a `dsl-completion/src/detectors/` (or similar) directory, extending
  the pattern `create_table.rs` / `create_index.rs` already establish
  but that wasn't applied consistently -- DDL variants like
  `create_transform_next_keyword`, `alter_default_privileges_next_
  keyword` etc. currently sit inline in `engine.rs` and should move
  out the same way index/table completions already did.

**Migration discipline:** move functions in small, themed groups (not
one-by-one across 210 functions, and not all at once) -- e.g. "GRANT/
REVOKE family", "window clause family", "CREATE ... family N". After
each group: `cargo test -p dsl-completion` (the full 19,243-line
suite) green, `cargo clippy -p dsl-completion --all-features --release
-- -D warnings` clean, commit. This mirrors the batch-by-batch,
verify-every-step rhythm the lint-rule expansion project used. Exact
grouping and batch count is a `writing-plans` decision made against
the real function list, not fixed here.

### Phase C -- Coverage audit and fill

Once the registry makes the engine's behavior legible, systematically
probe it against realistic incomplete queries across the three
coverage themes (advanced SQL surface, deeper phase-awareness,
cross-file intelligence), using the same empirical discipline as the
rule-expansion project: verify against the real engine before
committing to "this is a gap", not assumed gaps. `JSON_TABLE`
completion is one confirmed starting point; the audit will surface
more once the architecture stops being the bottleneck to reasoning
about it.

### Phase D -- Re-measure

Re-run the Phase A benchmark. The refactor alone -- replacing a
200+-branch linear scan with a registry walk that can short-circuit
and, later, be reordered by actual hit frequency -- should show some
improvement before any algorithmic tuning is attempted. Record before/
after numbers in the PR/commit description.

## Testing strategy

Phase B is pure extraction -- no new tests required beyond what
already exists, since behavior must not change; the existing suite is
the regression gate. Phase C additions get new tests following `dsl-
completion/tests/engine.rs`'s or `tests/context.rs`'s existing
conventions (whichever the specific addition fits), plus a probe
against the real engine binary/harness before finalizing each new
case, matching the empirical-verification discipline from the lint-
rule project.

## Out of scope / deferred

- Clean-slate rewrite (explicitly ruled out by the redesign-approach
  decision).
- Changing `dsl-analysis`, `dsl-hover`, `dsl-format`, `dsl-server`, or
  any other crate -- this plan is `dsl-completion` only.
- Splitting `dsl-completion/tests/engine.rs` (19,243 lines) is not
  mandated by this plan -- Phase B doesn't change test behavior, so
  splitting the test file is a separate, optional cleanup a future
  session could pick up, not required here.
- A precise, exhaustive list of every Phase C coverage gap -- that's
  the output of Phase C's audit, not an input to this spec.

## Success criteria

- Phase A: `perf_bench.rs` exists and runs, producing a baseline p50/
  p95.
- Phase B: `engine.rs` no longer contains the three-layer informal
  short-circuit structure; priority order lives in one place; full
  test suite green and clippy clean after every migration batch; zero
  behavior change (no test assertions rewritten to match new output --
  only moved/reorganized).
- Phase C: every newly added completion case has a test and was
  verified against the real engine before being called done.
- Phase D: benchmark re-run recorded; p50 target (< 5 ms, per the
  README) still holds.
