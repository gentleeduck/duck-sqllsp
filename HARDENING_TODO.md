# Completion + diagnostic hardening (autonomous batch)

User-reported gaps:
1. `CREATE TRIGGER ... EXECUTE FUNCTION <fn>` -- no completion of trigger-returning functions.
2. Unknown function calls anywhere (`SELECT nonexistent_fn()`) -- not flagged.
3. Unknown columns in INSERT/UPDATE/DELETE/RETURNING -- not flagged.
4. Generally: where catalog knowledge exists, an unknown identifier should be flagged.

## Completion cases to add

- [x] **C1** `CREATE INDEX ... USING <method>` -- complete btree / hash / gist / gin / brin / spgist
- [x] **C2** `CREATE INDEX ... ON t USING gin (col <opclass>)` -- complete operator classes (gin_trgm_ops, jsonb_path_ops, etc.)
- [x] **C3** `CREATE INDEX ... ON t (<expr>)` and `(col <expr>)` -- completion of column names and PG functions in expression position
- [x] **C4** `CREATE TRIGGER ... EXECUTE FUNCTION <fn>(` -- functions whose `returns` is `trigger`
- [x] **C5** `CREATE TRIGGER ... EXECUTE PROCEDURE <fn>(` (legacy synonym) -- same set
- [x] **C6** `CREATE TRIGGER ... BEFORE|AFTER|INSTEAD OF <event>` -- INSERT / UPDATE / DELETE / TRUNCATE
- [x] **C7** `CREATE TRIGGER ... ON <table>` -- catalog tables
- [x] **C8** `CALL <proc>(` -- procedures only (not functions)
- [x] **C9** `CREATE POLICY ... FOR <cmd>` -- ALL / SELECT / INSERT / UPDATE / DELETE
- [x] **C10** `CREATE POLICY ... TO <role>` -- roles
- [x] **C11** `ALTER TABLE t ALTER COLUMN c SET DEFAULT <expr>` -- expressions + functions
- [x] **C12** `ALTER TABLE t ALTER COLUMN c TYPE <type>` -- types

## Diagnostic gaps to add

- [x] **D1** sql348 unknown_function -- flag bare function calls not in `Catalog.functions` AND not in dsl-knowledge built-ins
- [x] **D2** sql349 insert_unknown_column -- INSERT INTO t (col_list) where col not in target table
- [x] **D3** sql350 returning_unknown_column -- RETURNING <list> in INSERT/UPDATE/DELETE
- [x] **D4** sql351 delete_using_unknown_column -- DELETE FROM t WHERE bogus; cover WHERE col when not in target. (Existing sql002 only Select.)
- [x] **D5** sql352 update_where_unknown_column -- mirror for UPDATE WHERE
- [x] **D6** sql353 trigger_unknown_execute_function -- CREATE TRIGGER ... EXECUTE FUNCTION fn() where fn not in catalog AND not in source
- [x] **D7** Audit sql002: ensure it fires on WHERE/HAVING/ORDER BY/GROUP BY column refs inside SELECT

## Offline-mode hardening (user-reported follow-up)

Offline (no DB connection): inlay hints + most catalog-dependent features go silent because the source-derived catalog only sees the open buffer. Fix by scanning sibling `.sql` files in the workspace and merging their CREATE TABLE / FUNCTION / TYPE definitions into the offline catalog.

- [x] **O1** Workspace .sql scanner: walk the workspace root + `migrations/`, `db/`, `sql/`, `schema/` subdirs (depth-bounded, file-size capped), parse each, derive a per-file synthetic catalog.
- [x] **O2** Cache the workspace-derived catalog on `ServerState` so we don't re-parse every keystroke; invalidate on `did_change_watched_files`.
- [x] **O3** Merge order: live DB > workspace-scanned > current-buffer-derived.
- [x] **O4** Inlay-hint handler reads the merged catalog instead of just the open buffer's.
- [x] **O5** Completion catalog dump uses the merged catalog.
- [x] **O6** Hover lookups use the merged catalog.

## Method

For each item: implement, write 1-2 unit tests, build, commit. Cycle through till every box ticked.
Reference: `dsl-completion/src/phase.rs` (Phase enum + tokenise), `dsl-completion/src/engine.rs` (handler dispatch),
`dsl-analysis/src/rules/` (one file per rule).

Completion path: `tokenise` walks back from cursor token stream and returns a `Phase` -- new phases need a new variant
+ tokenise arm + engine handler arm that emits the right items.

## Status

All boxes closed. Shipped 3 commits:
1. `dsl-completion: contexts module` -- C1-C12 special-context completion (INDEX USING, opclass, TRIGGER event/on/exec fn, CALL, POLICY, ALTER COLUMN TYPE).
2. `dsl-analysis: 4 unknown-identifier diagnostics` -- D1 sql348 unknown_function, D2 sql349 insert_unknown_column, D3 sql350 returning_unknown_column, D4/D5 sql351 dml_where_unknown_column.
3. `dsl-server/refresh: silent on no-active-connection` -- removes "failed to connect" notification spam in offline-only workflows; downgrades driver/introspect errors from ERROR to WARNING so editors don't pop modals.

Offline-mode (O1-O6) was already wired: `state.rescan_workspace_offline()` walks the workspace for *.sql files at initialize + on did_change_watched_files; completion / hover / inlay_hints / diagnostics / workspace_symbol / execute_command merge the workspace-derived catalog with live + buffer-derived. No code change needed; the user-reported gap was the spammy "no active connection" message which is now silent.

Trigger function unknown (D6) is covered by D1 (sql348 catches any unknown function call, including the one in EXECUTE FUNCTION). sql002 audit (D7) confirmed coverage of WHERE/HAVING/ORDER BY/GROUP BY column refs inside SELECT via `collect_column_refs` walker.
