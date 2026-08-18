# Phase C batch 1: aggregate/table-function syntax + catalog JSON keys

**Goal:** First Phase C coverage-audit batch. Empirically probe the
completion engine against realistic "advanced SQL surface" snippets
(the theme the brainstorming session prioritized), verify each finding
against the real engine via `complete_at()` before implementing, fix
confirmed gaps, add permanent regression tests.

**Method:** Same empirical-first discipline as the dsl-analysis
rule-expansion project -- write a throwaway probe test exercising
candidate snippets, read the *actual* completion output, only treat a
result as a confirmed gap once observed directly (not assumed from
reading detector code). Probe files were deleted before this commit;
findings and fixes are recorded here instead.

## Probed and confirmed correct (no gap, not touched)

- `ROWS BETWEEN <cursor>` -- offers `UNBOUNDED PRECEDING` / `CURRENT ROW`.
- `CREATE MATERIALIZED VIEW ... AS ... WITH <cursor>` -- offers `DATA` /
  `NO DATA`.
- `CREATE MATERIALIZED VIEW mv <cursor>` -- offers `USING`/`WITH`/
  `TABLESPACE`/`AS`.
- `COPY users FROM <cursor>` / `COPY users <cursor>` -- offers
  `STDIN`/`PROGRAM` and `FROM`/`TO` respectively.
- `LATERAL (SELECT ... WHERE u2.id = u.<cursor>` -- correctly resolves
  the outer alias's columns.
- 3-level-deep correlated subquery alias resolution -- correct.

## Confirmed gaps, fixed

1. **`agg(...) FILTER (WHERE ...)` not offered.** `FILTER` was missing
   from `sources::expression_keywords`'s keyword list (its sibling
   `OVER` was already there, unconditionally, matching this list's
   existing "broad expression-position keyword" philosophy). Fixed by
   adding it. Found in passing: the knowledge-base table
   (`dsl-knowledge/src/tables/keywords.rs`) had *two* separate
   `k!("FILTER", ...)` entries silently colliding in the same
   `HashMap` (the later one always won); removed the dead first one.

2. **`FILTER (<cursor>` dumped the full 788-item menu instead of just
   `WHERE`.** FILTER's entire grammar is `FILTER (WHERE <cond>)`, so
   WHERE is the only legal token immediately after the opening paren.
   Added `detectors::filter_clause_next_keyword`, modeled directly on
   the existing `tablesample_after_paren_next_keyword` pattern (narrow
   trigger: only fires with nothing typed yet inside the parens; once
   WHERE is present, falls through to normal expression completion --
   verified both ends empirically).

3. **`<table-fn>(...) WITH <cursor>` in FROM/JOIN didn't offer
   `ORDINALITY`**, jumping straight to the FROM-item-just-finished
   clause menu (JOIN/WHERE/ORDER BY/...) instead. Added
   `detectors::table_function_with_ordinality_next_keyword`. Guards
   against the one real ambiguity: `CREATE TABLE ... AS SELECT ...
   WITH <cursor>` also ends in `) WITH`, but means `WITH [NO] DATA`,
   not a table-function modifier -- excluded via a
   `starts_with("CREATE")` check on the current statement, verified
   the DATA/NO DATA completion still fires correctly there.

4. **`JSON_TABLE(... COLUMNS (<cursor>` at a fresh column-def slot**
   (right after the opening paren or after a `,`) surfaced an
   irrelevant catalog-table suggestion instead of recognising this is
   a brand-new name, not an entity to complete. Added
   `detectors::json_table_fresh_column_slot` (depth-counted from the
   *last* `COLUMNS` keyword, so nested `COLUMNS` lists inside `NESTED
   PATH ... COLUMNS (...)` are handled correctly too) -- suppresses the
   wrong dump and offers `FOR` (-> `<name> FOR ORDINALITY`).
   Deliberately narrow: does **not** attempt the rest of JSON_TABLE's
   column grammar (type keywords, `PATH`, `FORMAT JSON`, `EXISTS`,
   `NESTED`) -- verified that position (`COLUMNS (id <cursor>`) still
   falls through to the generic menu, an acceptable remaining gap
   rather than a regression, noted below.

5. **`json_path_keys_at_with_catalog` was implemented but never
   called** -- `complete_with_derived` only ever called the
   buffer-scan-only `json_path_keys_at`, so JSON key completion for
   `col->'<cursor>'` had no path to the catalog's `Column.json_keys`
   metadata (populated by a live DB's key survey) when the buffer had
   no example jsonb literal to harvest from. Fixed by switching the
   call site and moving the `catalog`/`derived` merge earlier in
   `complete_with_derived` (previously computed just before the
   dot-alias check, after the JSON-path slot; now computed immediately
   after offset normalisation -- the merge itself is cheap post the
   Phase D fix, so this reordering has no measurable cost).

## Known remaining gaps (not fixed this batch)

- ~~`JSON_TABLE(... COLUMNS (id <cursor>` (after a name is typed,
  before a type) -- falls through to the generic FROM-item-finished
  menu, not type-name completion.~~ **Fixed later** (2026-08-18, same
  day, on user request): `detectors::json_table_column_slot_items` +
  `engine::detect_json_table_column_slot` now cover
  name-typed -> types + FOR, `FOR` -> ORDINALITY, type-typed -> PATH/
  FORMAT/EXISTS, `FORMAT` -> JSON, `EXISTS` -> PATH. Still doesn't
  attempt multi-word types (`double precision`) or `NESTED ... COLUMNS`
  sub-lists' own grammar -- same "acceptable miss, not a wrong guess"
  scoping as the rest of this project. 6 new tests in
  `dsl-completion/tests/engine.rs`.
- Phase C's other two coverage themes (deeper phase-awareness beyond
  what this batch's probes happened to touch, and cross-file
  intelligence) are not yet systematically audited -- this batch
  focused on "advanced SQL surface" candidates found via the initial
  probe round.

## Tests

8 new tests in `dsl-completion/tests/engine.rs`:
`aggregate_call_offers_filter_keyword`, `filter_paren_offers_only_where`,
`filter_where_falls_through_to_normal_expression_menu`,
`table_function_offers_with_ordinality`,
`create_table_as_with_data_not_shadowed_by_ordinality`,
`json_path_completion_falls_back_to_catalog_json_keys`,
`json_table_columns_fresh_slot_offers_for_not_catalog_dump`,
`json_table_columns_fresh_slot_after_comma`.

`cargo test --workspace --release`: green (2039 dsl-completion tests,
0 failed, up from 2031). `cargo clippy --workspace --all-features
--release -- -D warnings`: clean.
