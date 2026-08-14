# Phase C batch 2: deeper phase-awareness + cross-file intelligence

**Goal:** Second Phase C coverage-audit batch, covering the two themes
batch 1 didn't reach. Same empirical-first discipline: probe against
the real engine, only fix confirmed gaps, verified before/after.

## Probed and confirmed correct (no gap, not touched)

Single-document phase-awareness, all confirmed correct via
`complete_at()`:
- Scalar subquery in the SELECT list, correlated to the outer query,
  resolving its *own* FROM table's columns.
- Resuming the normal SELECT-list menu after a `CASE ... END`
  expression.
- A CTE referencing an *earlier* CTE's projected columns via dot-alias
  (`WITH a AS (...), b AS (SELECT * FROM a WHERE a.<cursor>`).
- A correlated `IN (SELECT ...)` subquery resolving the outer alias.
- 3-level-deep nested correlated `EXISTS` subqueries (probed in batch
  1, still correct).

Lower-confidence observations, not treated as confirmed gaps this
batch (over-inclusive rather than wrong, or a niche feature -- noted
for a future pass if it turns out to matter in practice):
- `SELECT id FROM users UNION SELECT id FROM orders WHERE <cursor>`
  offers columns from *both* branches' tables rather than scoping to
  just the second SELECT's own FROM. `ORDER BY` after a `UNION`
  similarly doesn't prefer the first branch's projection aliases.
  Likely traces back to the same buffer-wide `fallback::scope_from_text`-
  style behavior already scoped to the current *statement* by the
  Phase D fix, not the current *sub-select* -- a real but smaller
  precision gap, deferred.
- A second `VALUES (...)` tuple's fresh slot offers `DEFAULT` plus the
  full function/keyword menu rather than type-aware literal
  suggestions -- reasonable fallback, not a broken case.

## Confirmed gaps, fixed

1. **A second (or later) window definition in a comma-separated
   `WINDOW w1 AS (...), w2 AS (<cursor>` clause fell through to the
   wrong menu** (FROM-item-finished keywords: JOIN/WHERE/ORDER BY/...)
   instead of the PARTITION BY / ORDER BY / frame-bound menu w1 (the
   *first* window) correctly gets. Root cause: both
   `detectors::window_clause_paren_expects_subclause` and
   `detectors::window_clause_partition_or_order_by_expects_column`
   located the window body via `after.find('(')` -- the position of
   the *first* `(` following the `WINDOW ` keyword -- rather than the
   paren actually still open at the cursor. Once w1's body closes and
   `w2 AS (` opens a second one, `find('(')` keeps returning w1's
   (already-closed) paren, so the depth check that followed operated
   on the wrong (and structurally confusing -- spanning w1's close and
   w2's open) slice.

   Fixed by replacing the "find the first paren" step in both
   functions with a proper single-pass depth scan that tracks the
   position of the *innermost still-open* paren as of the cursor
   (updates only on the depth 0->1 / 1->0 transitions, so a nested
   function call inside a window body like `PARTITION BY foo(x)`
   doesn't confuse it). Verified against both the new multi-window
   case and the existing single-window tests (`window_clause_paren_
   offers_partition_by`, `window_clause_partition_by_offers_columns`,
   plus the `r18_probe_window_specific` / `r2_036_window_paren_*`
   family) -- all still green.

2. **Completion did not see tables/types defined in *other open*
   (unsaved) documents.** `workspace_symbol.rs` already folds every
   open document's `derived_catalog()` together (`dsl-server/src/
   handlers/workspace_symbol.rs`), but `completion.rs`'s handler only
   ever merged in the *current* document's -- a table just typed in a
   sibling buffer, not yet saved to disk (so the on-disk workspace
   rescan hasn't picked it up either), was invisible until save.
   Verified empirically with a two-document dsl-server test:
   `CREATE TABLE widgets (...)` in `a.sql`, completion for `FROM widg`
   in `b.sql` returned nothing; the identical setup with both
   statements in one document returned `widgets` correctly, isolating
   the gap to cross-document visibility specifically.

   Fixed by folding every *other* open document's derived catalog into
   `completion.rs`'s `derived` value, mirroring `workspace_symbol.rs`'s
   pattern: this document's own `derived_catalog()` is the fold seed
   (so it wins on a same-name clash against a stale sibling
   definition), other open docs are filtered by `too_large()` the same
   way the current document already is, and the whole fold costs
   nothing extra in the common single-document-open case (zero other
   docs -> the fold runs zero iterations, seed returned unchanged --
   confirmed via a benchmark re-run: warm-cache completion still
   ~1.6ms/call at n=10,000, unchanged from the Phase D numbers).

## Tests

2 new tests: `second_window_def_in_comma_list_offers_subclause_menu`
in `dsl-completion/tests/engine.rs`;
`r5_202_completion_sees_table_defined_in_another_open_document` in
`dsl-server/tests/handlers_unit.rs`.

`cargo test --workspace --release`: green (dsl-completion 2040, up
from 2039; dsl-server 95, up from 94; 0 failed). `cargo clippy
--workspace --all-features --release -- -D warnings`: clean. Phase D
perf benchmark re-run: unchanged (~1.6ms/call warm-cache completion at
n=10,000).

## Remaining Phase C scope

Both themes now have at least one confirmed-and-fixed finding, plus a
few lower-confidence, deferred observations (UNION-branch column
scoping, VALUES-tuple type awareness) noted above rather than acted
on. Further batches, if pursued, would most productively start from
those deferred items or from a wider cross-file probe (e.g. a function
or custom type defined in one file and referenced in another) rather
than a from-scratch sweep -- the engine's coverage has proven solid
everywhere else probed so far across both batches.
