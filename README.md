<p align="center">
  <img src="./public/logo-dark.svg" alt="duck-sqllsp" width="120"/>
</p>

<h1 align="center">duck-sqllsp</h1>

<p align="center">
  A SQL language server that understands your schema.
</p>

<p align="center">
  <a href="https://crates.io/crates/duck-sqllsp"><img src="https://img.shields.io/crates/v/duck-sqllsp.svg" alt="crates.io"/></a>
  <a href="https://docs.rs/duck-sqllsp"><img src="https://docs.rs/duck-sqllsp/badge.svg" alt="docs.rs"/></a>
  <a href="./LICENSE"><img src="https://img.shields.io/crates/l/duck-sqllsp.svg" alt="MIT"/></a>
</p>

<p align="center">
  <a href="#install">Install</a> -
  <a href="#quick-start">Quick start</a> -
  <a href="#what-it-does">Features</a> -
  <a href="#configuration">Config</a> -
  <a href="dsl-server/docs/editors.md">Editor setup</a> -
  <a href="dsl-analysis/docs/rules.md">Rule reference</a>
</p>

---

Point your editor at a `.sql` file and you get completion that knows which
columns are on the table you joined, hover cards that show the real
`CREATE TABLE`, and 701 lint rules that catch the migration that will lock
your production table. It reads your schema from a live connection when you
give it one, and from the `.sql` files in your repo when you don't.

PostgreSQL is the deepest target. MySQL / MariaDB, SQLite, and SQL Server are
first-class for syntax, completion, hover, formatting, and introspection.
Written in Rust on tower-lsp and libpg_query.

## Install

```sh
cargo install duck-sqllsp
```

That gives you the `duck-sqllsp` binary. Any editor that can launch an LSP
server over stdio can use it. See [editor setup](dsl-server/docs/editors.md)
for VS Code, neovim, Helix, Emacs, and Sublime.

Using the crates as libraries instead:

```sh
cargo add dsl-analysis     # the 701-rule engine on its own
```

## Quick start

**neovim** (0.11+, stock `vim.lsp`):

```lua
vim.lsp.config('duck_sqllsp', {
  cmd = { 'duck-sqllsp', 'server' },
  filetypes = { 'sql', 'mysql', 'plsql' },
  root_markers = { '.duck-sqllsp.toml', '.duck-sqllsp.json', '.git' },
})
vim.lsp.enable('duck_sqllsp')
```

**VS Code**: grab the `.vsix` from the
[latest release](https://github.com/gentleeduck/duck-sqllsp/releases) and
`code --install-extension duck-sqllsp-vscode-*.vsix`. It adds sidebar trees for
connections and schema, and wires the Run / EXPLAIN code lenses to a terminal
running `psql` / `mysql` / `sqlite3`.

**No editor**: the same engine runs from the command line.

```sh
duck-sqllsp doctor                 # is the setup sane? start here when something is off
duck-sqllsp lint migrations/*.sql  # the analysis, without an editor in the loop
duck-sqllsp format file.sql --stdout
duck-sqllsp introspect file.sql    # catalog derived from CREATE TABLE/FUNCTION/TYPE
duck-sqllsp rules                  # every diagnostic code with a summary
```

`doctor` reports whether the external formatter is on `PATH`, which config file
was found, where the workspace root landed, how many `.sql` files the offline
scan reached, and whether the connection answers. It exits non-zero only on real
problems, so it doubles as a CI check.

## What it does

| | |
| --- | --- |
| **Lint** | 701 rules covering schema correctness, transaction safety, query smells, migration footguns, PL/pgSQL control flow, and vendor mismatches (MySQL `ENGINE=`, Oracle `DUAL`, T-SQL `BEGIN TRANSACTION`). Silence or re-level any of them by code. [Full reference](dsl-analysis/docs/rules.md) |
| **Completion** | ~50 context phases: `CREATE INDEX ... USING` and its opclass slot, `CREATE POLICY ... FOR / TO`, `ALTER COLUMN TYPE`, `CALL <proc>`, PL/pgSQL local scope, JOIN targets resolved through alias chains |
| **Hover** | Tables render a compact `CREATE TABLE` with indexes, triggers, policies, comments and owner. Columns, functions, keywords and types get their own cards, including NULL three-valued-logic notes |
| **Format** | `sql-formatter` reflow, DataGrip-style `CREATE TABLE` alignment, PL/pgSQL block indent. Formatting a selection snaps outward to whole statements, so the neighbours stay byte-identical |
| **Navigate** | Go-to-definition and type-definition across buffers, find references, rename, document and workspace symbols, folding, document links for psql `\i` includes and `COPY` paths |
| **Inline** | Column-name chips per `INSERT VALUES` tuple, `SELECT *` expansion, JOIN predicate suggestions, row estimates, and Run / EXPLAIN / `+ LIMIT 100` code lenses |
| **Refactor** | `= NULL` to `IS NULL`, `EXISTS (...)` to `CROSS JOIN LATERAL`, `IN` to `ANY`, extract a subquery into a CTE, and 30 more |
| **Offline** | No database needed. It walks `migrations/`, `db/`, `sql/`, `schema/` and builds a synthetic catalog. A live connection just makes it richer |

<details>
<summary>Details that matter on a large file</summary>

- **Incremental sync**: the editor ships only the edited range, spliced into the
  rope in place. On a 240 KB migration that is 200 bytes over 200 keystrokes
  rather than 48 MB, and about 4x less server work per edit.
- **Deferred completion docs** via `completionItem/resolve`: the per-keystroke
  response carries labels and no markdown. A `SELECT` projection drops from
  ~440 KB to ~209 KB on the wire and stops rendering 2321 entries that get
  thrown away.
- **Diagnostics** go out by push or by LSP 3.17 pull, whichever the client
  advertises, never both. A pull on an unchanged buffer answers `Unchanged`
  without re-running the engine.
- **Semantic tokens** colour the names a file is *creating*, which a catalog
  lookup alone cannot do, and `semanticTokens/range` colours just the viewport.

</details>

## Configuration

Drop a `.duck-sqllsp.toml` at your project root. Every key accepts `camelCase`
or `snake_case`, and the `[duck_sqllsp]` wrapper is optional.

Connection URLs carry a password, so keep real ones out of the file you commit:
point the example at a local throwaway database, or gitignore the config.

```toml
[duck_sqllsp]
activeConnection   = "local"
dialect            = "postgres"     # postgres / mysql / sqlite / mssql
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
singleLine      = true              # collapse DML to one line; leaves DDL intact

[[duck_sqllsp.connections]]
name = "local"
url  = "postgres://user:pass@localhost:5432/mydb"
```

- [Configuration reference](dsl-server/docs/configuration.md) - every setting, its default, and what it changes.
- [Editor setup](dsl-server/docs/editors.md) - VS Code, neovim, Helix, Emacs, Sublime.
- [Troubleshooting](dsl-server/docs/troubleshooting.md) - start with `duck-sqllsp doctor`.
- [Rule reference](dsl-analysis/docs/rules.md) - all 701 diagnostics by code.

## Performance

| Metric | Target |
| --- | --- |
| Completion p50 | < 5 ms |
| Diagnostics p50 | < 20 ms |
| Hover p50 | < 3 ms |
| Format p50 | < 30 ms |
| Cold start | < 50 ms |
| Memory idle | < 30 MB |
| Memory at 4 MiB file | < 150 MB |

## Workspace

Every crate is published and usable on its own.

| Crate | Role |
| --- | --- |
| [`dsl-parse`](https://crates.io/crates/dsl-parse) | Parser. libpg_query primary, sqlparser fallback for MySQL / SQLite / MSSQL |
| [`dsl-catalog`](https://crates.io/crates/dsl-catalog) | Schema model: tables, columns, constraints, indexes, triggers, policies, sequences, types, functions |
| [`dsl-knowledge`](https://crates.io/crates/dsl-knowledge) | Static keyword / type / function reference with links into the PG docs |
| [`dsl-resolve`](https://crates.io/crates/dsl-resolve) | Name resolution: FROM / JOIN / LATERAL scope, CTE columns, alias chains |
| [`dsl-format`](https://crates.io/crates/dsl-format) | Formatter: reflow, DataGrip alignment, PL/pgSQL indent, optional one-line DML |
| [`dsl-analysis`](https://crates.io/crates/dsl-analysis) | Lint engine, 701 diagnostics with narrow ranges |
| [`dsl-completion`](https://crates.io/crates/dsl-completion) | Completion engine, ~50 phases, alias and scope aware |
| [`dsl-hover`](https://crates.io/crates/dsl-hover) | Hover cards with cursor-side narrowing and schema-qualified resolution |
| [`dsl-conn`](https://crates.io/crates/dsl-conn) | Live PG / MySQL / SQLite introspection over sqlx |
| [`dsl-server`](https://crates.io/crates/dsl-server) | tower-lsp server, 25 request handlers plus startup progress |
| [`duck-sqllsp`](https://crates.io/crates/duck-sqllsp) | The binary: subcommands, stdio LSP, signal handling |

## How it is built

- **libpg_query** parses first, **sqlparser** picks up what it cannot, so MySQL
  backticks, T-SQL brackets and SQLite quirks all still parse.
- **tower-lsp** is the only place tokio touches. Every handler is a thin shim
  over a pure-function crate, which is why the crates are useful standalone.
- **Per-document parse cache** on `OnceLock`: the first heavy handler after
  `didChange` pays the parse, the rest reuse it.
- **Space-preserving strip** keeps 1:1 byte offsets while blanking strings,
  comments and dollar-quoted bodies, so every diagnostic range maps back exactly.
- **Catalog snapshots** are cloned out of a `parking_lot::RwLock` before any
  `.await`, so no guard ever crosses an await point.
- **`PR_SET_PDEATHSIG`** on Linux, plus SIGTERM / SIGINT / SIGHUP handling on
  Unix, so the binary dies with its editor rather than lingering.

## Contributing

```sh
cargo build --release
cargo test --workspace --release
cargo clippy --workspace --all-features --release -- -D warnings
```

2500+ tests across rules, completion phases, the hover resolver, the formatter
and the parsers. PR checklist and style notes are in
[`CONTRIBUTING.md`](CONTRIBUTING.md); see also [`SECURITY.md`](SECURITY.md) and
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## Sibling projects

[`@gentleduck/ui`](https://github.com/gentleeduck/duck-ui) -
[`@gentleduck/iam`](https://github.com/gentleeduck/duck-iam) -
[`@gentleduck/upload`](https://github.com/gentleeduck/duck-upload) -
[`@gentleduck/md`](https://github.com/gentleeduck/duck-mc)

## License

MIT. See [`LICENSE`](LICENSE).
