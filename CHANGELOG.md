# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com),
and the project adheres to [Semantic Versioning](https://semver.org).

## [Unreleased]

### Added

- **701 lint rules** (up from ~650): PL/pgSQL control flow and unused
  locals, `EXECUTE ... USING ... INTO` arity mismatches, declarative
  partitioning DDL, recursive-CTE restrictions, `MERGE` branch
  reachability, `GROUPING SETS` / `CUBE` / `ROLLUP`, correlated-subquery
  and join footguns, domain / composite / `EXCLUDE` / statistics-object
  mistakes, SQL-standard JSON functions, jsonpath and jsonb operators,
  and logical replication.
- `textDocument/diagnostic` (LSP 3.17 pull diagnostics). Push and pull
  share one analysis pass and are mutually exclusive per client, so
  nothing is rendered twice. Unchanged buffers answer `Unchanged`
  without re-running the engine.
- `textDocument/rangeFormatting`. The selection snaps outward to whole
  statements, since no sub-statement fragment survives being formatted
  in isolation.
- `textDocument/documentLink` for psql `\i` includes, `COPY ... FROM/TO`
  data files, and URLs in comments. File links are emitted only when the
  path resolves on disk.
- `completionItem/resolve`. Documentation is now rendered for the item
  you highlight rather than for every candidate.
- Semantic token modifiers (`declaration`, `definition`,
  `defaultLibrary`) and `semanticTokens/range`. Names introduced by
  `CREATE`, column and parameter lists, and PL/pgSQL locals are now
  classified -- previously they emitted no token unless they happened to
  match the live catalog.
- `duck-sqllsp doctor` -- reports what the server can actually see:
  formatter binary, which config file was found and whether it set
  anything, the derived workspace root, the size of the offline catalog,
  and connection health. Exits non-zero only on real problems, so it is
  safe in CI.
- Full [configuration reference](dsl-server/docs/configuration.md), a
  [troubleshooting guide](dsl-server/docs/troubleshooting.md), and a
  [lint rule reference](dsl-analysis/docs/rules.md), and
  [editor setup instructions](dsl-server/docs/editors.md) covering VS
  Code, neovim (both the 0.11+ and lspconfig paths), Helix, Emacs, and
  Sublime.

### Changed

- `CONTRIBUTING.md` and `SECURITY.md` rewritten for this project. Both
  were inherited from a sibling repository and described it instead:
  contributors were told to run `pnpm install` and
  `cargo test --features pretty-code` (a flag no package defines), and
  the security policy's entire threat model covered MDX sanitisation, a
  Node sidecar, and NAPI binaries -- none of which exist here. The
  security surfaces that do exist are now documented: connection
  credentials in the project file, read-only introspection queries, the
  code lenses that really execute SQL, the external formatter binary
  taken from `PATH`, and the bounded workspace file walk.
- `duck-sqllsp rules` now prints a one-line summary next to each code,
  and `--json` gains a `title` field. Previously it emitted only the
  code and severity, so a `sql015` in the editor could not be looked up
  without reading the source.

### Changed

- Document sync is now incremental. On a 240 KB file this is 0.016 ms
  per edit instead of 0.070 ms, and 200 bytes of traffic over 200
  keystrokes instead of 48 MB.
- Completion responses no longer carry documentation. A `SELECT`
  projection response drops from ~440 KB to ~209 KB, and 2321 built-in
  knowledge-base entries (~617 KB, ~1 ms) are no longer rendered per
  keystroke.

### Fixed

- **Crash on multi-byte characters.** The source scanners walked a byte
  cursor and sliced the string with it, so a CJK comment, an accented
  column name, or an emoji in a literal panicked with "byte index is not
  a char boundary". Because the workspace scan reads every `*.sql` file
  under the project root, one such file anywhere killed completion for
  every buffer. Fixed at 20 sites.
- **Config that set only `style` or only `rules` was silently ignored.**
  Both the JSON and the `.duck-sqllsp.toml` paths guessed whether a
  document was wrapped in `[duck_sqllsp]` by trying the wrapped parse and
  checking whether it produced a connection -- which is not a test of
  shape, since serde skips unknown fields and returns an all-defaults
  value. A wrapped config with no connection, and every bare
  `.duck-sqllsp.toml`, parsed "successfully" into nothing. No error was
  reported; settings just did not apply.
- Closing a document left its diagnostics in the client's problem panel
  and leaked its entry in the format cache, which holds a full copy of
  the buffer.
- The `MAX_DOC_BYTES` guard documented as protecting "heavy handlers" was
  honoured by only three of them; now applied to all whole-document
  handlers.
- 416 knowledge-base entries were silently shadowed by `HashMap::insert`
  collisions, with a panic-on-duplicate guard added so it cannot recur.
- `CREATE POLICY` expression columns were being reported as roles on
  hover.

### Internal

- CI had never run a test: the workflow invoked a `--features
  pretty-code` that no package in the workspace defines, so cargo exited
  before running anything. Spell-check and `cargo-deny` configuration
  were likewise inherited from another project and described its
  dependency graph rather than this one. All five jobs now pass.

## [0.1.0] -- 2026-05-26

First public release of duck-sqllsp -- a persistent SQL Language Server
for PostgreSQL (with MySQL introspection) built on `tower-lsp` +
`libpg_query`.

### Added

#### Language server (`dsl-server`, `duck-sqllsp` CLI)

- LSP 3.17 stdio server implementing:
  - `textDocument/completion` (context-aware, ~50 phases)
  - `textDocument/hover` (table / column / function / keyword cards)
  - `textDocument/signatureHelp` (function arg + INSERT column lists)
  - `textDocument/definition` (jump to CREATE TABLE / FUNCTION / TYPE)
  - `textDocument/references` (table / column refs across the workspace)
  - `textDocument/rename` (rename column / table across files)
  - `textDocument/codeAction` (>30 quick-fixes -- `= NULL` -> `IS NULL`,
    `EXISTS (...)` -> `CROSS JOIN LATERAL`, `BEGIN TRANSACTION` ->
    `BEGIN`, extract subquery to CTE, etc.)
  - `textDocument/formatting` and `textDocument/onTypeFormatting`
  - `textDocument/inlayHint` (suggested JOIN predicates)
  - `textDocument/documentSymbol`, `workspace/symbol`
  - `workspace/executeCommand` (`duck-sqllsp.testConnection`,
    `duck-sqllsp.getCatalog`)
- Offline mode: walks the workspace for `*.sql` files (workspace root +
  `migrations/`, `db/`, `sql/`, `schema/`) and derives a synthetic
  catalog so completion / hover / diagnostics work without a live
  database. Cached on `ServerState`; invalidated on
  `did_change_watched_files`.
- Live PG / MySQL introspection via `dsl-conn` (`sqlx`), merged with the
  workspace + open-buffer catalog (live > workspace > buffer).

#### Analysis (`dsl-analysis`)

- ~300 lint rules across the `sql001` -- `sql353` range covering:
  - Schema correctness (unknown table / column / function, missing FK,
    nullable PK, generated-from-volatile, etc.)
  - Transaction safety (DDL in transaction, advisory-lock literals,
    savepoint without release, REINDEX / CREATE INDEX in tx, etc.)
  - Query smells (`= NULL`, `NOT IN` over nullable column, `WHERE TRUE`,
    `LIMIT` without `ORDER BY`, `DISTINCT` after `GROUP BY`, etc.)
  - Migration footguns (`ALTER COLUMN TYPE` rewrite, `SET NOT NULL`
    scan, `ADD CHECK` without `NOT VALID`, etc.)
  - Vendor mismatches (MySQL `ENGINE=`, Oracle `DUAL` / `CONNECT BY`,
    SQL Server `BEGIN TRANSACTION`, etc.)
- Shared `textutil::strip_noise_full` (and the gentler
  `strip_comments_strings` / `strip_comments_only` variants) for
  comment / string / `$$`-block hijack-proof scanning.
- Severity levels (Error / Warning / Hint) and stable diagnostic codes
  suitable for `.sqllintignore` / per-rule disabling.

#### Completion (`dsl-completion`)

- Phase-based context detection covering >50 special positions:
  `CREATE INDEX ... USING`, opclass slot, expression-position columns +
  PG functions, `CREATE TRIGGER ... EXECUTE FUNCTION` (trigger-returning
  fns only), `CREATE POLICY ... FOR / TO`, `ALTER TABLE ... SET DEFAULT`
  / `TYPE`, `CALL <proc>`, PL/pgSQL local-variable scope, etc.
- Kind-classified items (Table / View / Column / Function / Keyword /
  Type / Schema) with `sort_priority`: in-scope cols first, then
  in-scope tables, scoped builtins, catalog tables / fns, keywords,
  catch-all.
- Source-derived completion: open-buffer `CREATE TABLE` / `FUNCTION` /
  `TYPE` definitions surfaced before any live-catalog ones.

#### Hover (`dsl-hover`)

- Hover cards for tables (column list with types, constraints, indexes),
  columns (type + parent + implicit specs from PK / UNIQUE / FK /
  SERIAL), functions (signature + docs), keywords (curated docs +
  examples + canonical PG URL), roles, types, NULL keyword (three-valued
  logic + inferred column nullability).
- Multi-word window-based resolution so hover hits the middle word of
  `INNER JOIN`, `IS NOT NULL`, etc.

#### Parse / resolve / catalog

- `dsl-parse`: `libpg_query` protobuf -> internal AST, covering all PG18
  syntax; unknown nodes preserved as `StatementKind::Unknown { text }`
  so the feature stack still sees raw SQL.
- `dsl-resolve`: scope / binding resolution with `FROM` / `JOIN` alias
  tracking, lateral-correlation awareness, and synthetic-binding
  injection.
- `dsl-catalog`: serialisable on-disk catalog (`schemas`, `functions`,
  `types`, `roles`) compatible across older snapshots via `serde`
  defaults.

#### Formatter (`dsl-format`)

- Whitespace-aware re-formatter with configurable `FormatterStyle`
  (language flavour) and `CreateTableStyle`.

### Editor integrations

- VS Code extension scaffold (`vscode-extension/`) with schema-tree view
  backed by `duck-sqllsp.getCatalog`.
- Neovim setup documented in README (works out of the box with built-in
  `vim.lsp` client and `nvim-cmp`).

### Notes

- All 95 hardening cycles closed (comment / string / `$$`-block
  hijack-proofing across every rule).
- 862 unit + integration tests, all green; `cargo clippy --all-targets
  -D warnings` clean.
- MSRV: Rust 1.90 (2024 edition).
