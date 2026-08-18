# Phase B batch 3: cleanup

**Grouping:** documentation only, no logic changes. Rewrote `engine.rs`'s
module doc to describe the new two-registry architecture (it still
described the pre-refactor "two-phase routing" shape). Confirmed no
leftover "must beat X" comments duplicate what moved into each
detector's own doc comment in batches 1-2 -- `route_phase` and
`complete_with_derived` are now just the registry loops plus (for
`route_phase`) the unchanged `Phase` match.

**Deliberately not done:** decomposing `contexts::detect()`'s 9
internal cases (`index_using_method`, `trigger_event`, ...) into
further top-level registry entries. Its outer gates
(`upper.contains("CREATE INDEX")` etc, computed once in `detect()`)
are load-bearing precision guards, not redundant wrapping -- e.g.
`index_using_method`'s own check (`rfind("USING ")`, no paren yet)
would false-positive on `DELETE ... USING other_table` without the
CREATE-INDEX gate ahead of it. Decomposing safely would mean every one
of the 9 new detectors re-deriving and re-checking that gate
individually -- real effort for a module that's a separate file, not
one of "engine.rs's 210 functions", and already participates in the
registry as one entry (`detect_contexts`) with its internal dispatch
unchanged. Left as a candidate for a future batch if ever warranted,
not attempted here.

**Verified unchanged:** doc-only change, so this batch is trivially
zero-behavior-change; ran the full suite anyway as the standing
per-batch gate.
