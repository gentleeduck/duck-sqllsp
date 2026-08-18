# Phase B registry refactor: overview and batch plan

**Goal:** Replace the "three informal short-circuit layers" (ad-hoc
`if`/`if let` chains in `complete_with_derived` and `route_phase`,
plus the separate `contexts::detect()` mechanism) with an explicit,
ordered detector registry -- "one array is the only place priority
order lives" -- per the design spec's Phase B section.

## Structural finding before extracting anything

The current control flow is genuinely **two stages**, not one:

1. **Pre-phase stage** (in `complete_with_derived`): fresh-name-slot
   suppression, JSON-path key slot, inert-span bailout, dot-context,
   grouping-sets-inner-paren, `contexts::detect()`. All run *before*
   the phase is even determined (`create_index::detect` /
   `create_table::detect` / `phase::detect`).
2. **Post-phase stage** (in `route_phase`): ~10 "slot-keyword
   shortcuts" that run *after* a phase (`ph`, `ix_phase`, or
   `ct_phase`) has been determined, regardless of which of the three
   determined it -- then the `Phase`-driven `match` as the final
   fallback.

Forcing both stages into one literal list would require reordering
detectors relative to `create_index::detect`/`create_table::detect`,
which changes real behavior (their outcome is independent of the
slot-keyword shortcuts' triggers, but I can't prove no realistic
snippet ever triggers both at once without exhaustive testing, and the
plan's own success criterion is *zero* behavior change). So: **two
registries**, one per stage, each independently "the only place
priority order lives" within its stage -- a faithful, safe reading of
the goal given the algorithm's actual two-stage shape.

Also found while mapping this: `route_phase`'s own
`grouping_sets_inner_paren_expects_column(source, offset)` check
(line ~450, before this batch) is **dead code**. `complete_with_derived`
already checks the identical `(source, offset)` pair earlier and
returns unconditionally if true (line ~372), and `source`/`offset`
are unmodified between the two calls, so by the time any of
`route_phase`'s 3 call sites are reached the condition is guaranteed
false. Confirmed via the call-site grep (only 3 callers, all
downstream of the first check) before removing it in batch 1.

## Batch plan

1. **Registry infrastructure + post-phase shortcuts.** Introduce the
   `Detector` type and a `run_detectors` loop; migrate `route_phase`'s
   ~10 slot-keyword shortcuts into a `POST_PHASE_DETECTORS` registry
   (dropping the dead grouping-sets duplicate). Most bounded, best
   understood piece -- good place to prove the mechanism works before
   applying it to the trickier pre-phase stage.
2. **Pre-phase stage, simple checks.** JSON-path key slot, inert-span,
   grouping-sets-inner-paren (the real, reachable one), `contexts::
   detect()` -- four independent, single-condition checks -- into a
   `PRE_PHASE_DETECTORS` registry.
3. **Pre-phase stage, fresh-name-slot cluster.** The `at_fresh_name_
   slot` gate plus its 8 inner checks, added to `PRE_PHASE_DETECTORS`.
   Kept as one detector wrapping the existing nested logic verbatim
   (not atomized further) -- the inner checks share an early-return
   structure that doesn't decompose into independent `Option<Vec<Item>>`
   detectors without changing the "return `Vec::new()` if nothing
   matched" fallback's meaning.
4. **Pre-phase stage, dot-context cluster.** Same treatment for
   `dot_alias` and its many inner branches -- one detector, existing
   logic unchanged, added last so `PRE_PHASE_DETECTORS` is complete in
   its current documented order: fresh-name-slot, JSON-path,
   inert-span, dot-context, grouping-sets, contexts.
5. **Cleanup.** Trim module docs to describe the registry shape,
   confirm `route_phase`'s remaining body is just the `Phase` match,
   remove now-redundant "must beat every phase variant" comments that
   have moved into registry-entry doc comments instead.

Every batch: `cargo test -p dsl-completion --release` 0 failed,
`cargo clippy -p dsl-completion --all-features --release -- -D
warnings` clean, one commit. No test assertions change -- this is
pure reorganization.
