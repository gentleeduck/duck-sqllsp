# Phase B batch 1: registry infrastructure + post-phase shortcuts

**Grouping:** the 10 "slot-keyword shortcuts" currently living at the
top of `route_phase` (before the `Phase` match), the most bounded and
already-enumerated piece of the refactor (see the overview doc). One
of the 10 -- `route_phase`'s own
`grouping_sets_inner_paren_expects_column` check -- is confirmed dead
(shadowed by an identical, always-first-reached check in
`complete_with_derived`) and is dropped rather than migrated.

**Shape:** `type Detector = fn(&str, TextSize, &ParsedFile, &[Scope],
&Catalog) -> Option<Vec<Item>>`. Each detector wraps one existing
shortcut's logic verbatim (including the three with an extra
trailing-token condition beyond the underlying helper's own `Some`/
`true`), named `detect_<thing>`. `const POST_PHASE_DETECTORS: &[Detector]`
lists them in their *exact current order* -- this is now the only
place that ordering lives, replacing the individual "must beat X"
inline comments (kept as each entry's own doc comment instead).
`route_phase` becomes: run the registry loop, `Some` short-circuits;
`None` from every entry falls through to the (unchanged) `Phase`
match.

**Verified unchanged:** same 9 conditions, same order, same keyword
lists / build logic per condition -- pure code motion into named
functions plus one dead-code removal.
