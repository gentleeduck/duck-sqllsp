# Phase B batch 2: full pre-phase detector registry

**Revised scope (was originally planned as 3 separate batches -- see
the overview doc's original batches 2/3/4):** building
`PRE_PHASE_DETECTORS` incrementally across several commits would mean
the array is in the *wrong* order for however many commits it takes
to add every entry -- and "one array is the only place priority order
lives" only actually holds once every entry is present in the right
order. The six pre-phase checks interleave (fresh-name-slot, JSON-path,
inert-span, dot-context, grouping-sets, contexts::detect, in that
exact sequence), so this batch does all six at once instead.

**Grouping:** every pre-phase check from the top of
`complete_with_derived`, in current order:
1. fresh-name-slot cluster (gate + 8 inner keyword checks + empty
   fallback) -- wrapped as **one** detector, internals verbatim. The
   inner checks share an early-return structure building toward
   "return `Vec::new()` if nothing matched"; decomposing them into
   independent detectors would change what "the gate didn't match"
   means for each one, so this is motion, not further atomization
   (same treatment batch 1 gave the 3 compound-condition shortcuts,
   just at a larger scale).
2. JSON-path key slot
3. inert-span bailout
4. dot-context cluster (gate + many inner branches sharing a mutable
   `out`) -- same one-detector treatment as #1.
5. grouping-sets-inner-paren (the real, reachable one -- distinct from
   the dead duplicate batch 1 removed from `route_phase`)
6. `contexts::detect()`

**Shape:** same `Detector` signature as batch 1. New
`const PRE_PHASE_DETECTORS: &[Detector]` holding all six, in order.
`complete_with_derived` becomes: normalise offset, merge catalog, loop
the registry (first `Some` wins), then the two remaining steps that
were never part of the shortcut chain -- `create_index::detect` /
`create_table::detect` phase overrides, then `phase::detect` +
`route_phase`.

**Subtlety carried over correctly:** the inert-span bailout means
"yes, claim this position, answer is empty" (`Some(Vec::new())`), not
"decline" (`None`) -- same reasoning as the original plan note.
Likewise the fresh-name-slot cluster's final `return Vec::new()` (when
the gate matched but none of the 8 inner checks did) becomes
`Some(Vec::new())`, not `None` -- the *gate* matching is what commits
to "I'm claiming this slot", regardless of which (if any) inner
keyword check fires.

**Verified unchanged:** same 6 conditions, same order, same internals.
