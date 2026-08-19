<p align="center">
  <img src="./public/logo-dark.svg" alt="duck-sqllsp" width="120"/>
</p>

<h1 align="center">duck-sqllsp</h1>

<p align="center">
  Persistent multi-dialect SQL Language Server. PostgreSQL is the deepest target; MySQL / MariaDB, SQLite, and SQL Server are first-class for syntax, completion, hover, formatting, and connection-backed introspection. Built on tower-lsp + libpg_query.
</p>

<p align="center">
  <a href="./LICENSE">MIT</a> -
  <a href="./CHANGELOG.md">Changelog</a> -
  <a href="./CONTRIBUTING.md">Contributing</a> -
  <a href="./dsl-cli">Crate docs</a>
</p>

<p align="center">
  <a href="https://crates.io/crates/duck-sqllsp"><img src="https://img.shields.io/crates/v/duck-sqllsp.svg" alt="crates.io"/></a>
  <a href="https://docs.rs/duck-sqllsp"><img src="https://docs.rs/duck-sqllsp/badge.svg" alt="docs.rs"/></a>
  <a href="./LICENSE"><img src="https://img.shields.io/crates/l/duck-sqllsp.svg" alt="MIT"/></a>
</p>

---

## What you get

- **701 lint rules** (PG-first, dialect-aware) covering schema correctness, transaction safety, query smells, migration footguns, vendor mismatches (MySQL `ENGINE=`, Oracle `DUAL`/`CONNECT BY`, SQL Server `BEGIN TRANSACTION`, ...). Delivered by push (`publishDiagnostics`) or by LSP 3.17 pull (`textDocument/diagnostic`), whichever the client advertises -- never both, so nothing renders twice. Pull requests on an unchanged buffer answer `Unchanged` without re-running the engine.
- **Context-aware completion** across ~50 phases: `CREATE INDEX ... USING` + opclass slot, `CREATE TRIGGER ... EXECUTE FUNCTION`, `CREATE POLICY ... FOR / TO`, `ALTER COLUMN TYPE`, `CALL <proc>`, PL/pgSQL local-variable scope, JOIN target resolution, etc.
- **Document links**: psql `\i` / `\ir` / `\include` / `\include_relative` targets, `COPY ... FROM/TO '<file>'` data files, and URLs in comments are all clickable. File links are emitted only when the path actually resolves on disk — a `COPY` path interpreted by the *server* usually doesn't exist locally, and a link that reliably errors is worse than none.
- **Incremental document sync**: the editor ships only the edited range, spliced into the rope in place. On a 240 KB migration file that's 200 bytes over 200 keystrokes instead of 48 MB, and ~4x less server-side work per edit (0.016 ms vs 0.070 ms).
- **Deferred completion docs** via `completionItem/resolve`: the per-keystroke response ships labels and no markdown, and documentation is rendered only for the item you actually highlight. A `SELECT` projection completion drops from ~440 KB to ~209 KB on the wire, and stops rendering markdown for 2321 built-in entries (~617 KB, ~1 ms) that get thrown away.
- **Rich hover cards** for tables (compact `CREATE TABLE` + indexes + triggers + policies + comment + `ALTER TABLE OWNER TO`), columns, functions, keywords, types, NULL three-valued logic notes.
- **Offline mode** built in: walks the workspace for `*.sql` files (`migrations/`, `db/`, `sql/`, `schema/`) and derives a synthetic catalog so completion + hover + diagnostics work without a live DB. Live introspect (PG / MySQL / SQLite) overrides.
- **Code lenses**: `Run` above every statement + `EXPLAIN` for DML; `+ LIMIT 100`, `EXPLAIN ANALYZE` for slow SELECTs. VS Code wires them through to the active connection's CLI in a terminal; other clients see no broken-command popups.
- **Inlay hints**: column-name chip per INSERT VALUES tuple (DataGrip-style), `SELECT *` expansion, `JOIN ... ON ...` predicate suggestion, `-- ~N rows` at end of SELECT, literal-cast `::int` hint in WHERE.
- **Formatter**: external `sql-formatter` v15+ reflow + DataGrip-style `CREATE TABLE` alignment + PL/pgSQL block indenter. Optional `singleLine` post-pass collapses DML statements onto one line while leaving DDL intact. Format-selection (`textDocument/rangeFormatting`) snaps outward to whole statements, so formatting a caret inside one `CREATE TABLE` leaves its neighbours byte-identical.
- **Semantic tokens** with modifiers: `CREATE` targets carry `declaration`/`definition`, column and parameter lists inside a `CREATE` body are highlighted as declared properties/parameters (a plain catalog lookup can't colour a table the file is *creating*), and built-in types/functions get `defaultLibrary` so they read differently from your own. `semanticTokens/range` colours just the viewport.
- **Refactors / code actions**: `= NULL` -> `IS NULL`, `EXISTS (...)` -> `CROSS JOIN LATERAL`, `BEGIN TRANSACTION` -> `BEGIN`, extract subquery to `WITH _tmp AS (...) CTE`, 30+ more.
- **Editor integrations**: VS Code extension with connections + schema tree view, neovim setup that works with stock `vim.lsp` + `nvim-cmp`.

## Install

```sh
cargo install duck-sqllsp
```

Library use:

```sh
cargo add dsl-analysis
```

## Quick start

```lua
-- neovim
vim.lsp.config('duck_sqllsp', {
  cmd = { 'duck-sqllsp', 'server' },
  filetypes = { 'sql', 'mysql', 'plsql' },
  root_markers = { '.duck-sqllsp.toml', '.duck-sqllsp.json', '.git' },
})
vim.lsp.enable('duck_sqllsp')
```

```sh
duck-sqllsp --help
duck-sqllsp version
duck-sqllsp doctor                     # check formatter, config, workspace, connection
duck-sqllsp rules
duck-sqllsp lint file.sql
duck-sqllsp format file.sql --stdout
duck-sqllsp introspect file.sql        # offline catalog from CREATE TABLE/FUNCTION/TYPE
duck-sqllsp introspect --url postgres://user:pass@host/db
```

Project config example (`.duck-sqllsp.toml`):

```toml
[duck_sqllsp]
activeConnection   = "local"
dialect            = "postgres"     # postgres / mysql / sqlite / mssql (aliases accepted)
requireConnection  = false

[duck_sqllsp.rules]
sql015 = "off"                      # silence a rule by code

[duck_sqllsp.style]
keywordCase    = "upper"
functionCase   = "lower"
typeCase       = "upper"
identifierCase = "preserve"

[duck_sqllsp.style.formatter]
expressionWidth = 100
singleLine      = true              # collapse DML to one line; leaves DDL untouched

[[duck_sqllsp.connections]]
name = "local"
url  = "postgres://user:pass@localhost:5432/mydb"
```

Every key accepts `camelCase` or `snake_case`, and the `[duck_sqllsp]`
wrapper is optional.

- **[Configuration reference →](dsl-server/docs/configuration.md)** — every setting, its default, and what it changes.
- **[Troubleshooting →](dsl-server/docs/troubleshooting.md)** — start with `duck-sqllsp doctor`.
- **[Lint rule reference →](dsl-analysis/docs/rules.md)** — all 701 diagnostics by code, with what each one catches.

## Workspace

| Crate | Role |
| --- | --- |
| [`dsl-parse`](dsl-parse) | SQL parser - libpg_query primary, sqlparser fallback for MySQL/SQLite/MSSQL |
| [`dsl-catalog`](dsl-catalog) | Schema model - tables (incl. owner), columns, constraints (inline + table-level), indexes, triggers, policies, sequences, types, functions |
| [`dsl-knowledge`](dsl-knowledge) | Static keyword / type / function reference with PG-doc links |
| [`dsl-resolve`](dsl-resolve) | Name resolution, FROM / JOIN / LATERAL scope, CTE columns, alias chains |
| [`dsl-format`](dsl-format) | Formatter - sql-formatter reflow + DataGrip alignment + PL/pgSQL indent + optional one-line DML pass |
| [`dsl-analysis`](dsl-analysis) | Lint rule engine - 701 diagnostics with narrow ranges |
| [`dsl-completion`](dsl-completion) | Context-aware completion engine, ~50 phases, alias + scope aware |
| [`dsl-hover`](dsl-hover) | Hover cards with cursor-side narrowing, schema-qualified resolution |
| [`dsl-conn`](dsl-conn) | Live PG / MySQL / SQLite catalog introspection (sqlx) |
| [`dsl-server`](dsl-server) | tower-lsp server - 25 LSP request handlers + startup progress |
| [`dsl-cli`](dsl-cli) | `duck-sqllsp` binary - subcommands + stdio LSP + signal handling |

## Editor integrations

- **VS Code**: install `wildduck.duck-sqllsp-vscode`. Sidebar tree views for connections + schema. Commands: Add Connection, Set Active, Test Connection, Refresh Schema, Restart Server, Show Logs. Run / EXPLAIN / EXPLAIN ANALYZE / + LIMIT 100 code lenses wire through to a `duck-sqllsp` terminal running `psql` / `mysql` / `sqlite3` against the active connection.
- **neovim**: stock `vim.lsp` + `nvim-cmp`. duck-sqllsp emits `$/progress` so statusline plugins surface "loading workspace..." while the .sql scan + DB introspect settle.

## Build

```sh
cargo build --release
cargo test  --workspace --release
cargo clippy --workspace --all-features --release -- -D warnings
```

2500+ tests (rules, idioms, completion phases, hover resolver, formatter, parsers).

## Performance targets

| Metric | Target |
| --- | --- |
| Completion p50 | < 5 ms |
| Diagnostics p50 | < 20 ms |
| Hover p50 | < 3 ms |
| Format p50 | < 30 ms |
| Memory idle | < 30 MB |
| Memory @ 4 MiB file | < 150 MB |
| Cold start | < 50 ms |
| Document update | incremental, no re-parse on cached handlers |

## Design

- **libpg_query** primary parser, **sqlparser** fallback so MySQL backticks, MSSQL bracketed idents, SQLite quirks all parse.
- **tower-lsp** protocol layer. Every handler is a thin shim over a pure-function crate; the LSP transport is the only place tokio touches.
- **Per-document parse cache** on `OnceLock` - first heavy handler after `didChange` pays the parse cost, the rest reuse it.
- **Space-preserving strip** keeps 1:1 byte offsets when stripping strings / comments / dollar-quoted bodies, so diagnostic ranges map back to source byte-exact.
- **Catalog snapshots** are `parking_lot::RwLock` reads cloned before any `.await` - no guard ever crosses an await point.
- **`PR_SET_PDEATHSIG`** + SIGTERM / SIGINT / SIGHUP handling - the binary always dies with its editor.

## Sibling repos

[`@gentleduck/ui`](https://github.com/gentleeduck/duck-ui) -
[`@gentleduck/iam`](https://github.com/gentleeduck/duck-iam) -
[`@gentleduck/upload`](https://github.com/gentleeduck/duck-upload) -
[`@gentleduck/md`](https://github.com/gentleeduck/duck-md)

## Contributing

PR checklist + style notes in [`CONTRIBUTING.md`](CONTRIBUTING.md).
Security: [`SECURITY.md`](SECURITY.md). Behaviour: [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## License

MIT. See [`LICENSE`](LICENSE).
