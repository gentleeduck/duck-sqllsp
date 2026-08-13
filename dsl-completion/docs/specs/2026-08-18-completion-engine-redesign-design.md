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

**Update from Phase A (measured, not hypothetical):** now that
`dsl-completion/tests/perf_bench.rs` exists, the target is confirmed
missed, and the reason is now known. On a 10,000-statement buffer,
`complete()` averages **~25ms/call -- 5x the < 5ms target**. A second
benchmark (`perf_scaling_is_position_independent_not_size_
independent`) isolated why: per-call cost is the same whether the
cursor sits near the start or the end of the buffer (ruling out
"distance scanned back to buffer start"), but grows with *total*
statement count -- roughly `0.65ms + 0.0025ms * n_statements` across
n = 200/1000/3000. Every `complete()` call is doing work proportional
to the whole buffer, not to local cursor context. On a large
migrations file this means every keystroke's completion request gets
slower as the file grows, regardless of where in the file you're
typing. This sharpens Phase D's success criterion below: eliminate
this whole-buffer scaling, not just move the same per-call cost into
better-organized files.

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
  any other crate -- this plan is `dsl-completion` only. **Exception
  made during Phase D** (see the Phase D findings below): the
  redundant-rescan fix genuinely required touching `dsl-server`'s
  `Document`/`ParseCache` (there is no way to eliminate cross-handler
  redundant computation from inside `dsl-completion` alone -- the
  redundancy exists *between* dsl-server's handlers, not within any
  one call), so that boundary was crossed deliberately and narrowly:
  one new cached field + accessor in `documents.rs`, and the 5
  existing call sites updated to use it instead of recomputing. One
  pre-existing `dsl-analysis` clippy warning (unrelated `question_mark`
  lint, blocking the `-D warnings` gate) was also fixed in passing,
  consistent with how this session has handled every other
  pre-existing clippy warning it happened to hit. `dsl-hover` was
  found to have its own, larger, unrelated perf issue during Phase D
  (see below) and was deliberately left untouched -- that fix is a
  separate, comparably-sized effort outside this plan's scope.
- Splitting `dsl-completion/tests/engine.rs` (19,243 lines) is not
  mandated by this plan -- Phase B doesn't change test behavior, so
  splitting the test file is a separate, optional cleanup a future
  session could pick up, not required here.
- A precise, exhaustive list of every Phase C coverage gap -- that's
  the output of Phase C's audit, not an input to this spec.

## Success criteria

- Phase A (done): `perf_bench.rs` exists and runs, producing a
  baseline. Confirmed the < 5ms target is currently missed by ~5x at
  10k statements, and confirmed the cause is whole-buffer-size
  scaling, not cursor-position scaling -- see the spec update above
  and the commit history for exact numbers.
- Phase B: `engine.rs` no longer contains the three-layer informal
  short-circuit structure; priority order lives in one place; full
  test suite green and clippy clean after every migration batch; zero
  behavior change (no test assertions rewritten to match new output --
  only moved/reorganized). Additionally -- since the refactor is the
  natural place to fix this -- `complete()`'s cost should stop scaling
  with total buffer/statement count: `perf_scaling_is_position_
  independent_not_size_independent`'s per-call numbers at n=3000
  should drop toward the n=200 numbers, not stay at ~8ms.
- Phase C: every newly added completion case has a test and was
  verified against the real engine before being called done.
- Phase D (done, 2026-08-18): both `perf_bench.rs` tests re-run and
  numbers recorded; the whole-buffer scaling is gone -- see "Phase D
  findings" below for the root cause, the fix, and before/after
  numbers. `complete()` at n=10,000 (uncached, the benchmark's
  worst case): ~25ms/call -> ~2.9ms/call, under the < 5ms p50 target.

## Phase D findings (2026-08-18)

Two independent perf issues were found and fixed. The first
(`Document::derived_catalog` caching in dsl-server, eliminating
redundant `source_tables::from_source` calls across completion / hover
/ diagnostics / inlay-hints / workspace-symbol for the same document
version) was the one this plan anticipated going in -- real, but it
turned out to be a small slice of the total: standalone `from_source`
at n=10,000 costs ~1.6ms, a fraction of the original ~25ms/call.

The actual dominant cost, found only by instrumenting
`complete_with_derived` with per-branch `Instant` timing after fix #1
alone left the benchmark essentially unchanged (the humbling part --
the initial root-cause hypothesis, based on reading the code rather
than measuring it, was wrong): for a cursor sitting in a WHERE clause
whose table isn't recognized by the catalog (no live DB, no workspace
scan, no CREATE TABLE in the buffer -- exactly `perf_bench.rs`'s
`Catalog::default()` setup, and a completely ordinary state for a
fresh session or a schema-less scratch file), completion falls back to
`fallback::scope_from_text`. Its `iter_table_bindings` helper collected
the *entire source buffer* into a `Vec<char>` and scanned it
start-to-end for every FROM / JOIN / UPDATE / INTO / USING in the
*whole file*, regardless of cursor position -- O(buffer size), not
O(statement size), which is exactly why the original Phase A benchmark
found the cost identical whether the cursor sat near the start or the
end of the buffer (a clue that, in hindsight, pointed away from
`from_source` -- a position-independent *and* buffer-size-dependent
cost, on every fallback-triggering call, not just once per edit).

Fixed by a new `engine::current_statement_span(source, offset)` helper
(mirrors the existing `stmt_slice_upper`'s semicolon-boundary
convention, extended to find the end boundary too) and scoping every
`fallback::{scope_from_text, cte_names_from_text, cte_columns_from_text}`
call site (4 in total, across `engine.rs` and `detectors.rs`) to the
current `;`-delimited statement instead of the whole buffer. The two
CTE-fallback functions had a latent *correctness* bug from the same
cause, not just a perf one: both only look at their argument's leading
prefix (per their own doc comments), so passing the whole buffer meant
they were checking statement #1's prefix for a `WITH` clause instead
of the current statement's -- scoping to the current statement fixes
both.

Numbers (n=10,000 statements, cursor at end-of-statement matching
`build_buffer`'s convention -- see `dsl-completion/tests/perf_bench.rs`
and `dsl-server/tests/handlers_unit.rs`'s
`r5_201_perf_derived_catalog_cache_avoids_redundant_rescans` for full
detail and reproduction steps):

| Path | Before | After |
|---|---|---|
| `complete()` uncached (perf_bench.rs, worst case) | ~25ms/call | ~2.9ms/call |
| dsl-server completion, warm cache | ~25ms/call (no caching existed) | ~1.6ms/call |
| dsl-server completion, cold cache (first call after an edit) | ~25ms/call | ~74ms one-time (pays parse + resolve + derive; unchanged from before, since parse/resolve were already paid once per edit via the pre-existing `ParseCache`) |

Separate finding, not fixed here: `hover::run` was measured at
~270ms/call at n=10,000 during this investigation -- one to two orders
of magnitude worse than everything else, and unrelated to both fixes
above. Root cause: `dsl_hover::hover_with` takes raw `source: &str` and
calls `dsl_parse::parse` on it internally on *every* call, ignoring
dsl-server's already-cached `ParseCache` entirely -- confirmed
pre-existing, not caused by this session. Left untouched per the
out-of-scope note above; flagged for a future, `dsl-hover`-scoped
follow-up (thread a pre-parsed `&ParsedFile` through `hover_with`
instead of re-parsing, the same shape of fix as this session's `derived_catalog`
work, just in a different crate).
