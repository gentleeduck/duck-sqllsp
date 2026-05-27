# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com),
and the project adheres to [Semantic Versioning](https://semver.org).

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
