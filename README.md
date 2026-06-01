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

- **300+ lint rules** (PG-first, dialect-aware) covering schema correctness, transaction safety, query smells, migration footguns, vendor mismatches (MySQL `ENGINE=`, Oracle `DUAL`/`CONNECT BY`, SQL Server `BEGIN TRANSACTION`, ...).
- **Context-aware completion** across ~50 phases: `CREATE INDEX ... USING` + opclass slot, `CREATE TRIGGER ... EXECUTE FUNCTION`, `CREATE POLICY ... FOR / TO`, `ALTER COLUMN TYPE`, `CALL <proc>`, PL/pgSQL local-variable scope, JOIN target resolution, etc.
- **Rich hover cards** for tables (compact `CREATE TABLE` + indexes + triggers + policies + comment + `ALTER TABLE OWNER TO`), columns, functions, keywords, types, NULL three-valued logic notes.
- **Offline mode** built in: walks the workspace for `*.sql` files (`migrations/`, `db/`, `sql/`, `schema/`) and derives a synthetic catalog so completion + hover + diagnostics work without a live DB. Live introspect (PG / MySQL / SQLite) overrides.
- **Code lenses**: `Run` above every statement + `EXPLAIN` for DML; `+ LIMIT 100`, `EXPLAIN ANALYZE` for slow SELECTs. VS Code wires them through to the active connection's CLI in a terminal; other clients see no broken-command popups.
- **Inlay hints**: column-name chip per INSERT VALUES tuple (DataGrip-style), `SELECT *` expansion, `JOIN ... ON ...` predicate suggestion, `-- ~N rows` at end of SELECT, literal-cast `::int` hint in WHERE.
- **Formatter**: external `sql-formatter` v15+ reflow + DataGrip-style `CREATE TABLE` alignment + PL/pgSQL block indenter. Optional `singleLine` post-pass collapses DML statements onto one line while leaving DDL intact.
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
duck-sqllsp rules
duck-sqllsp lint file.sql
duck-sqllsp format file.sql --stdout
duck-sqllsp introspect file.sql        # offline catalog from CREATE TABLE/FUNCTION/TYPE
duck-sqllsp introspect --url postgres://user:pass@host/db
```

Project config example (`.duck-sqllsp.toml`):

```toml
[duck_sqllsp]
active_connection = "local"
dialect           = "postgres"      # postgres / mysql / sqlite / mssql (aliases accepted)
require_connection = false

[duck_sqllsp.style]
keyword    = "upper"
function   = "lower"
type       = "upper"
identifier = "preserve"

[duck_sqllsp.style.createTable]
alignColumns       = true
openParenOnNewLine = true
constraintsAtEnd   = true

[duck_sqllsp.style.formatter]
language       = "postgresql"
expressionWidth = 9999
singleLine      = true              # collapse DML to one line; leaves DDL untouched
denseOperators  = true

[[duck_sqllsp.connections]]
name = "local"
url  = "postgres://user:pass@localhost:5432/mydb"
```

## Workspace

| Crate | Role |
| --- | --- |
| [`dsl-parse`](dsl-parse) | SQL parser - libpg_query primary, sqlparser fallback for MySQL/SQLite/MSSQL |
| [`dsl-catalog`](dsl-catalog) | Schema model - tables (incl. owner), columns, constraints (inline + table-level), indexes, triggers, policies, sequences, types, functions |
| [`dsl-knowledge`](dsl-knowledge) | Static keyword / type / function reference with PG-doc links |
| [`dsl-resolve`](dsl-resolve) | Name resolution, FROM / JOIN / LATERAL scope, CTE columns, alias chains |
| [`dsl-format`](dsl-format) | Formatter - sql-formatter reflow + DataGrip alignment + PL/pgSQL indent + optional one-line DML pass |
| [`dsl-analysis`](dsl-analysis) | Lint rule engine - 300+ diagnostics with narrow ranges |
| [`dsl-completion`](dsl-completion) | Context-aware completion engine, ~50 phases, alias + scope aware |
| [`dsl-hover`](dsl-hover) | Hover cards with cursor-side narrowing, schema-qualified resolution |
| [`dsl-conn`](dsl-conn) | Live PG / MySQL / SQLite catalog introspection (sqlx) |
| [`dsl-server`](dsl-server) | tower-lsp server - all 17 LSP request handlers + startup progress |
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
