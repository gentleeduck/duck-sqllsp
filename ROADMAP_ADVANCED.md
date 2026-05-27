# duck-sqllsp -- Advanced Semantic Analysis + Remaining Gaps

Captured 2026-05-24. Living document.

---

## A. Variable-typed-as-table -> column completion

**Goal**: when the user writes

```sql
DECLARE
    r users;       -- composite (row) type, same shape as the table
    promo promo_codes;
BEGIN
    r.|              -- here, complete with users columns
```

the dot context after `r.` should surface the columns of `users` even
though `r` is a local variable, not a FROM-binding.

### Implementation steps

1. **Extend `dsl-completion/src/plpgsql_locals.rs`** to extract the
   declared type alongside the local name (already extracts names; add
   a `type_name: Option<String>` field on `Local`).
2. **Treat a local whose `type_name` is a known catalog table** as a
   synthetic scope binding pointing at that table. Inject it into the
   resolver's `Scope` when computing PL/pgSQL body completion.
3. **Dot context resolver** (engine.rs `dot_alias` path): when alias not
   in FROM scope, look it up in the plpgsql locals table; if its type
   resolves to a catalog table, emit that table's columns.
4. **Hover**: same trick -- when cursor on `r.id`, render as `users.id`.
5. **Tests**:
   - `DECLARE r users; ... r.<TAB>` lists users columns.
   - `r.email` hover -> `users.email` card.
   - Negative: `DECLARE r INT;` -> no row-style completion.
   - Composite types from `CREATE TYPE addr AS (street ...)` resolve too.

### Estimated effort
~150 lines + 5 tests. Half a session.

---

## B. Advanced semantic analysis -- function return correctness

**Goal**: catch real semantic errors at edit time instead of waiting
for `psql` to choke.

### Rules to add (one per concern, in dsl-analysis)

| Code | Severity | Catches |
|------|----------|---------|
| sql030 | Error | `RETURNS TRIGGER` without `RETURN NEW;` / `RETURN OLD;` / `RETURN NULL;` in body. |
| sql031 | Error | `RETURN <expr>;` where expr type ≠ declared `RETURNS <type>`. |
| sql032 | Error | `RETURN;` (bare) inside a function that declares non-void return type. |
| sql033 | Error | `RETURNS TABLE(col t, ...)` body has no `RETURN QUERY` / `RETURN NEXT` matching the row shape. |
| sql034 | Error | `RETURN <table_name>` where the table's column shape doesn't match the declared row type. |
| sql035 | Warning | Trigger function references `NEW` in a `BEFORE DELETE` trigger (NEW is NULL there) -- or `OLD` in `BEFORE INSERT`. |
| sql036 | Warning | `RAISE EXCEPTION` with format string but missing positional args (`%` count mismatch). |
| sql037 | Warning | `SELECT INTO var` where `var` row shape doesn't match the SELECT projection. |
| sql038 | Error | `INSERT INTO t (a, b) VALUES (1)` -- column count vs value count mismatch. |
| sql039 | Error | `INSERT INTO t (a, b) VALUES (1, 'x')` where column type doesn't accept the literal. |
| sql040 | Warning | Function declared `IMMUTABLE` calls a `VOLATILE` function (purity violation). |
| sql041 | Warning | `LANGUAGE sql` function body references `NEW` / `OLD` (only valid in plpgsql). |
| sql042 | Error | `UPDATE` SET clause assigns to column that doesn't exist. |
| sql043 | Warning | `DELETE` without `WHERE` inside a function that's not explicitly marked unsafe. |
| sql044 | Error | `EXIT` / `CONTINUE` outside a LOOP block. |
| sql045 | Warning | Unreachable code after unconditional `RETURN` / `RAISE EXCEPTION`. |

### Infrastructure needed

1. **Type system**: a small `Type` enum in dsl-analysis (Int, Numeric,
   Text, Uuid, Bool, Date, Timestamp, Timestamptz, Jsonb, Table(name),
   Record, Unknown). Inferred from literal shape + catalog column types.
2. **Function signature index**: parse all `CREATE FUNCTION` bodies in
   the buffer + live catalog, store (params, return_type, language).
3. **PL/pgSQL body walker**: shallow control-flow over IF/LOOP/RETURN
   branches. Doesn't need full SSA -- branch-reaches-end analysis is
   enough for sql030/sql045.
4. **Catalog hooks for type coercion**: which casts are implicit (int ->
   numeric ok, text -> int not). Use a static table for v1.

### Estimated effort
- Type enum + inference: 1 day
- Function signature index: 0.5 day
- Each rule sql030..sql045: 2-4 hours
- Tests per rule: ~10 cases
- **Total**: ~4-5 days for full set.

### Sequencing
- Tier 1 (high impact, low cost): sql030, sql032, sql038, sql044
- Tier 2 (needs type inference): sql031, sql033, sql037, sql039, sql042
- Tier 3 (needs control flow): sql045, sql035, sql036, sql040, sql041, sql043

---

## C. Everything else still missing from the LSP

### Drivers (P1 leftover)

| Driver | Status | Effort |
|--------|--------|--------|
| MSSQL / T-SQL | not started; needs tiberius (not in sqlx) | 1 day |
| Oracle | not started; oracle crate behind feature flag | 1 day |
| DuckDB | not started; native bindings | 0.5 day |
| Snowflake | not started; HTTP-based, no driver | 1 day |
| BigQuery | not started; HTTP-based | 1 day |

### Parser (tree-sitter-sql forked, 2 gaps left)

| Issue | Fix difficulty |
|-------|----------------|
| `expr::TEXT[]` cast to array type -- subscript ambiguity | Medium (grammar refactor for type in cast position) |
| `SELECT ... WHERE ...` without FROM | Easy (split `from` rule, move WHERE to select-level) |
| PL/pgSQL `EXCEPTION` handler block | Medium |
| PL/pgSQL cursor declarations (`DECLARE c CURSOR FOR ...`) | Medium |
| PL/pgSQL `FOR ... IN ... LOOP` | Medium |
| Window function frame clauses inside OVER() partial | Easy |

### Completion gaps

- JSON path keys (`metadata->>'|'`) -- would need sample-row sniffing
- Composite column projection in CTE body (we expose CTE name only)
- Subquery aliasing (`FROM (SELECT ...) AS t(a, b)`) -- parser doesn't
  surface the column list yet
- Function call argument hints (signature_help already there; tie to
  cursor position over each arg better)
- Window function `OVER (PARTITION BY |)` completion of grouping cols

### Hover gaps

- Trigger / sequence / policy when cursor is on their name as the target
- `::cast` operator hover explaining the source -> target conversion
- Constraint clause hover (`PRIMARY KEY`, `CHECK`, `EXCLUDE`)
- PL/pgSQL `tg_op` / `tg_when` / `tg_level` / `tg_name` magic var hovers

### Formatter gaps

- PL/pgSQL body formatter -- currently passes through sql-formatter raw.
  Aligning IF/THEN/RAISE/GET DIAGNOSTICS like hand-written code needs
  a dedicated pass (extend dsl-format with `plpgsql_align.rs`).
- Configurable per-keyword indent table (hardcoded right now)
- CASE WHEN multi-line alignment
- Comment preservation (sql-formatter strips some attached comments)

### Hardening gaps

- Fuzz parser harness (cargo-fuzz) -- pin crashes/panics
- Property tests for ct_align beyond idempotency (no token loss,
  monotonic length growth, structure preservation)
- Catalog cache invalidation on schema-change events (currently full
  refresh)
- LSP request cancellation tokens (start a check, abandon if next edit
  arrives before completion)
- Memory cap per concurrent request
- Structured error telemetry with file location

### Performance gaps

- Incremental parsing -- full re-parse per edit today; tree-sitter
  incremental would cut latency on big files
- Catalog lazy-load per schema instead of eager dump
- Completion item dedup is O(n²) in the worst case (HashSet helps)
- Hover precompute on idle (so first hover after focus isn't blocking)
- Document sync uses FULL not INCREMENTAL -- switch to incremental
- Catalog persistence: load from disk on init, refresh in background

### UX gaps

- Code actions: extract subquery, inline CTE, swap LEFT/INNER JOIN,
  add explicit cast, rename alias
- Inlay hints: parameter names at call sites, inferred types after `:=`,
  resolved alias on bare column refs
- Workspace symbol: faster fuzzy filter, jump-to-trigger
- Rename: handles columns + tables; does it survive across files?
- Document highlight: same alias / same column refs all underlined
- Selection range: expand to statement, then block, then file
- Folding ranges: each top-level statement, each plpgsql block,
  each constraint group

### Testing gaps

- End-to-end LSP test harness (spawn server, send JSON-RPC, assert)
- Snapshot tests for formatter golden output
- Fuzz harness for parser
- Concurrency tests (multiple docs + multiple requests interleaved)
- Memory leak tests
- Property tests for resolver scope (every alias resolves to its bound
  table; bound table exists in catalog or is a CTE)

### Documentation gaps

- README badly out of date with respect to the crate split
- Per-crate rustdoc at lib-level on every public item
- ARCHITECTURE.md walking the data flow per request type
- CONTRIBUTING with the lint-rule template
- User-facing config schema doc (settings.json options)

---

## D. Prioritized next-session work

If forced to pick one, do them in this order:

1. **A** (variable-typed-as-table completion) -- small, high DX win
2. **B Tier 1** (sql030 missing-RETURN-NEW, sql032, sql038, sql044) -- 1 day
3. **Hardening: cargo-fuzz harness** -- catches latent panics
4. **Incremental parsing** -- perf win across the board
5. **C completion / hover gaps for trigger / sequence / policy**
6. **B Tier 2** (with the type inference engine)
7. **MSSQL driver** when a user requests it
8. **B Tier 3** + control-flow analysis
9. Code actions / inlay hints
10. Documentation overhaul

---

## E. Open design questions

- **Catalog change propagation**: should a `LISTEN` thread refresh the
  catalog when the DB schema changes? Or rely on explicit `:DBRefresh`?
- **Type inference confidence**: when type can't be inferred (e.g.
  `now()` return type), do we silently skip, warn, or fail-closed?
- **Cross-file resolution**: do we want a workspace-wide index of every
  `CREATE FUNCTION` so `SELECT my_fn()` in file A jumps to file B?
- **`sqlfluff` integration**: should advanced rules borrow from
  sqlfluff's rule set, or stay native?
- **Multi-dialect rules**: rule code prefixes should encode dialect
  (`pg-sql030`, `mysql-sql030`)? Or keep a single namespace and let the
  rule decide?

---

End of roadmap. Update this file at the end of each iteration so the
next session has a single source of truth.
