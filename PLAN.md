# duck-sqllsp -- Architectural Plan

A modern, feature-rich SQL Language Server built in Rust. Targets the same
"feels like a real LSP" bar that rust-analyzer and tsserver set: incremental,
diagnostics-first, schema-aware, and well-integrated with nvim-cmp,
neovim's built-in LSP client, vim.diagnostic, telescope, and Trouble.

This document is the build plan, not the implementation. It locks down the
crate split, public traits, data model, and milestones so each crate can be
built and reviewed independently.

---

## 1. Mission

Give a developer editing SQL the same fluency they get editing Rust or
TypeScript in a modern IDE:

- **Live schema awareness**: tables, columns, types, constraints, indexes,
  and functions pulled from the actual database in the background.
- **Rich completion**: kind-classified items (table / view / column /
  function / keyword / type / schema), with signatures, doc strings,
  curated examples, and links to canonical Postgres docs.
- **Pre-execution validation**: catch unresolved table references, unknown
  column names, JOIN columns that don't exist, ambiguous columns, type
  mismatches, missing WHERE on UPDATE/DELETE, and 30+ other static checks
  *before* the query touches the database.
- **Hover that explains**: hover on a table shows its column list with
  types; hover on a column shows type + parent + constraints; hover on a
  keyword shows docs + example + URL.
- **Find references, go-to-definition, rename** across the queries in the
  workspace, scoped by the active connection's schema.

---

## 2. Non-goals

- Not a replacement for psql / DBeaver / DataGrip's query runners. We
  surface diagnostics and completion; we do not become a query result UI.
  (The user's neovim already has dadbod-ui for that.)
- Not a SQL parser for every dialect at v1. Postgres first, MySQL second,
  SQLite third. Dialects are pluggable but the team focus is Postgres.
- Not a migration tool. We don't generate ALTER TABLE statements.
- Not a query planner / optimizer hinting tool at v1 (deferred to v2).

---

## 3. Architecture overview

```
                                 +---------------------+
                                 |  neovim / VS Code   |
                                 +----------+----------+
                                            | LSP (JSON-RPC stdio)
                                            v
                                 +---------------------+
                                 |  dsl-server         |   tower-lsp-server
                                 |  request router     |   capability matrix
                                 +----------+----------+
                                            |
              +-----------------------------+-----------------------------+
              |             |               |              |              |
              v             v               v              v              v
       +------+-----+ +-----+-----+  +------+------+ +-----+-----+ +------+------+
       | dsl-       | | dsl-      |  | dsl-hover   | | dsl-      | | dsl-format  |
       | completion | | analysis  |  | rich        | | resolve   | | wraps       |
       | menu items | | lint rules|  | markdown    | | aliases + | | external    |
       |            | |           |  |             | | scopes    | | formatter   |
       +-----+------+ +-----+-----+  +-----+-------+ +-----+-----+ +-------------+
             |              |              |              |
             +------+-------+--------------+--------------+
                    |
                    v
            +-------+--------+              +---------------------+
            |  dsl-parse     |              |  dsl-catalog        |
            |  facade picks  |              |  schema cache       |
            |  pg_query OR   |              |  XDG-persisted JSON |
            |  sqlparser     |              +----------+----------+
            +-------+--------+                         |
                    |                                  v
        +-----------+------------+         +-----------+----------+
        v                        v         |  dsl-conn            |
   +----+-----+            +-----+------+  |  sqlx async drivers  |
   | pg_query |            | sqlparser  |  |  postgres/mysql/     |
   | (real PG |            | (Rust,     |  |  sqlite              |
   | parser,  |            | multi-     |  +----------------------+
   | vendored)|            | dialect)   |
   +----------+            +------------+
```

Build philosophy:
- We do not write a SQL parser. `sqlparser` (mature multi-dialect, used by
  Datafusion and Ballista) and `pg_query` (wraps the real Postgres
  parser source) shoulder the heavy lifting.
- We do not write DB drivers. `sqlx` covers postgres / mysql / sqlite.
- We focus on the LSP layer, name resolution, lint rules, completion,
  hover, and schema cache.

Incremental computation: kept simple. SQL files are small (< 5KB
typical, < 50KB extreme); a full re-parse on every edit is microseconds.
We cache per-document `(text_hash) -> parsed_ast` in a `DashMap`. No
salsa needed for v1. Add later if benchmarks demand it.

---

## 4. Crate layout

Cargo workspace under `crates/`. Slimmer than a hand-rolled parser would
require because `sqlparser`, `pg_query`, and `sqlx` cover the heavy
lifting.

| Crate            | Role |
|------------------|------|
| `dsl-knowledge`  | Static curated knowledge: keyword docs, signatures, examples, canonical Postgres docs URLs. Single source of truth shared by completion, hover, and diagnostics. No deps beyond std + serde. |
| `dsl-parse`      | Thin facade. Picks `pg_query` for Postgres files and `sqlparser` for MySQL / SQLite / generic SQL. Normalises both AST shapes into our internal `Statement` enum so downstream crates see one shape. Surfaces parser errors as `ParseError { span, message }`. |
| `dsl-resolve`    | Name resolution over our `Statement` enum. Builds a `Scope` per query: FROM tables, JOIN tables, CTEs, subqueries. Resolves every column reference to a `(table, column)` pair when possible; flags ambiguity. |
| `dsl-conn`       | Wraps `sqlx`. Trait `Driver { async fn introspect(&self) -> Catalog; async fn ping() -> Result<()>; }`. Concrete impls behind Cargo features: `postgres`, `mysql`, `sqlite`. Pool reuse via `sqlx::Pool`. |
| `dsl-catalog`    | Schema cache: `Schema -> Table -> Column / Constraint / Index`, plus `Function` table. Loads from `dsl-conn::introspect`, persists to `XDG_CACHE_HOME/duck-sqllsp/catalogs/<conn-id>.json` (versioned). Reactive: `workspace/didChangeConfiguration` triggers refresh. |
| `dsl-analysis`   | Diagnostics. Each rule is a `LintRule { fn check(&self, stmt: &Statement, scope: &Scope, catalog: &Catalog) -> Vec<Diagnostic>; }`. 27 built-in rules (sql001..sql027). Some rules are dialect-gated. |
| `dsl-completion` | Context detector + item builder. Consumes the parsed statement, the resolved scope, the catalog, and the cursor offset. Emits LSP `CompletionItem`s with `kind`, `detail`, `labelDetails`, `documentation` (markdown), `additionalTextEdits` for auto-injected JOIN clauses. |
| `dsl-hover`      | Hover markdown renderer. Resolves the token under the cursor (alias, table, column, keyword, function, type) and renders rich markdown: column lists for tables, type + parents + constraints for columns, signature + doc + example + URL for keywords. |
| `dsl-format`     | Formatting. Strategy: shell out to the user's existing `sql-formatter` + `sql_format.py` pipeline (we already have it). Fallback: pure-Rust `sqlformat` crate. Async via `tokio::process`. |
| `dsl-server`     | LSP wire layer (`tower-lsp-server`). Capability matrix, request routing, document store, change tracking, `workspace/executeCommand` dispatch (refreshSchema, switchConnection, validate). |
| `dsl-cli`        | Binary `duck-sqllsp`. Subcommands: `server` (default, LSP over stdio), `lint <file>`, `format <file>`, `introspect <url>` (one-shot catalog dump as JSON), `version`. |
| `dsl-test`       | Snapshot fixtures + integration harness spinning up LSP against a docker-compose Postgres. Reused by every crate's integration tests. |

Workspace `Cargo.toml` lists all crates with `path = "crates/<name>"`.
Binary lives in `crates/dsl-cli/src/main.rs` so `cargo install duck-sqllsp`
just works.

---

## 5. LSP capability matrix

| Capability                              | v1 | v2 | Notes |
|-----------------------------------------|----|----|-------|
| `textDocument/completion`               | yes |    | context-aware, kind-classified, rich docs |
| `completionItem/resolve`                | yes |    | lazy load column lists for table items |
| `textDocument/hover`                    | yes |    | table -> column list; column -> type/constraints |
| `textDocument/signatureHelp`            | yes |    | function arg hints from signatures |
| `textDocument/publishDiagnostics`       | yes |    | unresolved refs, type errors, lint rules |
| `textDocument/definition`               | yes |    | navigate from column to its declaring CREATE TABLE |
| `textDocument/references`               | yes |    | every usage of a table/column in workspace |
| `textDocument/rename`                   | yes |    | rename column/table across workspace files |
| `textDocument/documentSymbol`           | yes |    | outline view (statements + table defs) |
| `workspace/symbol`                      | yes |    | search tables/columns across workspace |
| `textDocument/codeAction`               | yes |    | quick fixes for diagnostics |
| `textDocument/semanticTokens`           | yes |    | table-vs-column highlighting LSP-driven |
| `textDocument/formatting`               | yes |    | DataGrip-style alignment via dsl-format |
| `textDocument/rangeFormatting`          | yes |    | same, scoped |
| `textDocument/inlayHint`                |    | yes | result row types, parameter types |
| `textDocument/foldingRange`             | yes |    | statement folds, CTE folds, block comments |
| `textDocument/selectionRange`           | yes |    | smart selection up the AST |
| `workspace/executeCommand`              | yes |    | switchConnection, refreshSchema, runQuery |
| `textDocument/codeLens`                 |    | yes | "Run", "Explain" lenses above statements |
| `textDocument/diagnostic` (pull model)  | yes |    | for clients that prefer pull |
| `workspace/willRenameFiles`             |    | yes | propagate file renames |

Server announces a `ServerCapabilities` matching v1 yes column on initialize.

---

## 6. Schema introspection

`dsl-catalog` is the single source of truth.

**Sources of truth in priority order:**
1. Live DB connection via `dsl-conn` (`information_schema` + `pg_catalog`).
2. Cached file at `XDG_CACHE_HOME/duck-sqllsp/<connection-id>.json` if the
   connection is unreachable or stale.
3. CREATE TABLE statements in the workspace files (last-resort, partial).

**Refresh policy:**
- On `initialize`: read cache file (instant), kick off live refresh in
  background.
- On `workspace/didChangeConfiguration` carrying a new active connection:
  invalidate cache, refresh in background, send progress notifications via
  `$/progress`.
- Manual: `workspace/executeCommand` with `duckSqllsp.refreshSchema`.
- Stale-after: 30 minutes by default, configurable.

**Schema model:**
```rust
pub struct Schema  { id, name }
pub struct Table   { id, schema, name, kind: Table|View|MatView,
                     columns: Vec<Column>, constraints: Vec<Constraint>,
                     indexes: Vec<Index>, comment: Option<String> }
pub struct Column  { id, table, name, ty: SqlType, nullable, default,
                     comment: Option<String> }
pub struct Function{ id, schema, name, args: Vec<Param>,
                     returns: SqlType, doc: Option<String> }
```

Identifiers (`Id`) are stable indices in arenas owned by the catalog so
HIR can cheaply reference them.

**Introspection queries:** SQL templates per dialect under
`dsl-conn-<driver>/queries/`. Postgres uses `pg_class`, `pg_attribute`,
`pg_constraint`, `pg_index`, `pg_proc` for richer detail than
`information_schema`.

**Failure mode:** if no DB is reachable, all completion / hover still
works against the cached catalog. Diagnostics that need fresh data emit
an `Info` diagnostic on the file's first line saying "catalog is stale".

---

## 7. Parser, name resolution, types

**No hand-written parser.** `dsl-parse` delegates to existing battle-tested
crates and exposes a unified output.

**Parser selection per dialect:**
| Dialect    | Primary           | Fallback         |
|------------|-------------------|------------------|
| Postgres   | `pg_query`        | `sqlparser` (pg) |
| MySQL      | `sqlparser` mysql | -                |
| SQLite     | `sqlparser` sqlite| -                |
| Generic    | `sqlparser` ansi  | -                |

- `pg_query` wraps the actual Postgres parser source code (vendored libpg_query). Identical syntax tree and error messages to the real database.
- `sqlparser` is a pure-Rust multi-dialect parser maintained for Datafusion / Ballista. Mature, fast.

**Normalisation.** `dsl-parse` owns a single `Statement` enum mirroring
the LSP-visible structure (`Select`, `Insert`, `Update`, `Delete`,
`CreateTable`, `AlterTable`, etc.). Both upstream ASTs are converted into
it. Downstream crates depend only on `dsl-parse::Statement`, isolating
them from upstream churn.

**Span mapping.** Both parsers expose byte ranges. We normalise to
`text_size::TextRange` so LSP ranges convert cleanly.

**Error tolerance.** `sqlparser` is not error-tolerant -- a single bad
token aborts. We slice the file on top-level semicolons before parsing
each statement, so a broken statement doesn't poison the rest. Per-statement
errors are surfaced as parser diagnostics (sql000 family).

**Name resolution (dsl-resolve):**
- Build a scope tree per statement from `Statement`.
- FROM / JOIN clauses populate the visible table set.
- WITH clauses introduce CTE scopes that nest.
- ColumnRef without a qualifier searches all visible tables; ambiguity
  becomes diagnostic sql003.
- Subqueries inherit the outer scope (for correlated subqueries).

**Types.** Deferred to v0.3. v0.1 / v0.2 use the catalog's column types
as opaque strings for hover display; no inference yet. v0.3 introduces a
lightweight type table covering Postgres scalar types.

---

## 8. Completion engine (dsl-completion)

**Inputs:** HIR + position. The position carries the cursor's location in
the syntax tree, plus the partial token under it.

**Context classifier:** decides what kinds of items make sense here.
Examples:

| Cursor position             | Items                                    |
|-----------------------------|------------------------------------------|
| After `SELECT`              | columns of visible tables, functions, `*` |
| Inside a SELECT list, qualified `t.<cursor>` | only columns of resolved alias `t` |
| After `FROM`                | tables, CTE names, subquery placeholder  |
| After `JOIN`                | tables, CTE names                        |
| Inside `ON <cursor>`        | columns of the two joined sides          |
| After `WHERE`               | columns, functions, operators            |
| After `ORDER BY`            | columns from SELECT list, functions      |
| Inside DDL `<type>` slot    | type names (UUID, TIMESTAMPTZ, ...)      |
| Inside parens of a function call | parameter types, function arg hints  |
| Start of statement          | top-level keywords                       |

**Item builders:** each produces a `CompletionItem` with:
- `label` (display)
- `kind` (LSP kind: Class for table, Field for column, etc.)
- `detail` (right-aligned label, e.g. `varchar(255)  users`)
- `labelDetails.description` (compact tag)
- `documentation` (markdown with signature + doc + example + URL)
- `insertText` (verbatim or snippet, e.g. function calls insert `name($1)`)
- `additionalTextEdits` (e.g. when picking `orders.user_id` after `JOIN`,
  also inject `ON orders.user_id = users.id`)
- `commitCharacters`
- `sortText` (rank: exact prefix > resolved-scope > workspace > general)

**Resolve hook:** `completionItem/resolve` is used to lazily attach
column lists to table items so the initial completion response stays
small.

**Filter:** completion items are filtered server-side by prefix; cmp
will additionally fuzzy-filter client-side. We expose `triggerCharacters`
= `[".", " ", "(", ","]` so menus open without explicit `<C-Space>`.

---

## 9. Hover engine (dsl-hover)

Inputs: HIR + position. Output: markdown.

Rules:
- Table -> column list table, then constraints (PK, FK targets, CHECK).
- View -> defining query (truncated) + column list.
- Column -> type, nullability, default, comment, parent table FQN.
- Function -> signature + doc + example + URL.
- Keyword -> doc + example + URL (from `dsl-knowledge`).
- Type name -> doc + URL.
- Built-in operator (`->>`, `::`, ...) -> doc + URL.
- Parameter placeholder -> type if inferable from context.

Format follows the cmp / hover convention of `kind: name -- one-liner`
header + body. Markdown for code blocks.

---

## 10. Diagnostics pipeline (dsl-analysis)

Diagnostics produced after every successful HIR build. Each rule is a
`LintRule` impl run over the resolved HIR. Severity levels:

| Level   | When |
|---------|------|
| Error   | The query will fail at runtime (unresolved table, unknown column, ambiguous column without qualifier, type mismatch, syntax error). |
| Warning | The query will run but probably do something wrong (UPDATE / DELETE without WHERE, SELECT * in production code rules, implicit cross-join, NULL comparison with `=`, missing GROUP BY columns). |
| Info    | Style and readability (lowercase keywords, missing aliases, redundant subquery, suggested LIMIT). |
| Hint    | Refactor suggestions (use LATERAL, replace IN with EXISTS). |

**Rule catalog (v1 baseline):**

| Code | Severity | Rule |
|------|----------|------|
| sql001 | Error | Unresolved table or view reference |
| sql002 | Error | Unknown column |
| sql003 | Error | Ambiguous column without table qualifier |
| sql004 | Error | Type mismatch in comparison |
| sql005 | Error | Type mismatch in INSERT VALUES vs target columns |
| sql006 | Error | INSERT column list arity mismatch |
| sql007 | Error | NOT NULL column without DEFAULT inserted with NULL |
| sql008 | Error | GROUP BY references non-existent column |
| sql009 | Error | Aggregate function used outside GROUP BY/HAVING/SELECT |
| sql010 | Error | Subquery returns more columns than parent expects |
| sql011 | Error | ORDER BY references unknown column when SELECT is DISTINCT |
| sql012 | Error | Recursive CTE without a UNION ALL terminator |
| sql013 | Warning | UPDATE or DELETE without WHERE |
| sql014 | Warning | Implicit cross-join (`FROM a, b` without WHERE join condition) |
| sql015 | Warning | Comparison with NULL using `=` or `<>` (use IS NULL) |
| sql016 | Warning | SELECT * in production-tagged file |
| sql017 | Warning | Missing GROUP BY columns referenced in SELECT |
| sql018 | Warning | Identical JOIN ON condition on both sides |
| sql019 | Warning | Mixing implicit FROM and explicit JOIN |
| sql020 | Warning | Deprecated function used |
| sql021 | Info  | LIMIT missing on SELECT in production-tagged file |
| sql022 | Info  | Table referenced but never selected (in INSERT INTO only) |
| sql023 | Info  | Redundant DISTINCT (already on a PK) |
| sql024 | Info  | UNION can be UNION ALL (no duplicates expected) |
| sql025 | Hint  | Use EXISTS instead of IN (subquery returns single column) |
| sql026 | Hint  | LATERAL JOIN can replace correlated subquery |
| sql027 | Hint  | Replace COUNT(col) with COUNT(*) when col is NOT NULL |

Each diagnostic carries `code`, `source = "duck-sqllsp"`, a primary
range, and zero or more `relatedInformation` entries (e.g. for ambiguous
column, point at every candidate). Code actions (sql004 `cast value to T`,
sql015 `replace with IS NULL`, sql026 `convert to LATERAL`) are attached.

**Pre-execution validation:** EXPLAIN-based validation runs on demand
(via `workspace/executeCommand` `duckSqllsp.validate`) -- sends the query
through `EXPLAIN (FORMAT JSON)` against the active connection and surfaces
planner errors as diagnostics. Not on every keystroke; only when invoked.

---

## 11. Configuration

**LSP `initializationOptions`:**
```jsonc
{
  "duckSqllsp": {
    "connections": [
      {
        "name": "local",
        "driver": "postgres",
        "url": "postgres://user:pass@localhost:5432/app",
        "default": true
      }
    ],
    "activeConnection": "local",
    "schemaRefreshSeconds": 1800,
    "completion": {
      "preferQualified": false,
      "snippetFunctions": true,
      "rankTablesAbove": ["sessions", "logs"]
    },
    "diagnostics": {
      "rules": { "sql013": "error", "sql016": "off" }
    },
    "format": {
      "preset": "datagrip-aligned",
      "tabWidth": 4
    }
  }
}
```

Reloadable via `workspace/didChangeConfiguration`.

**Per-project file** at `<workspace>/.duck-sqllsp.toml` overrides editor
settings. Same shape, TOML.

**Connection secrets** (passwords) never live in the LSP config files;
they come from env vars referenced by `${ENV:PG_PASSWORD}` patterns, or
from the OS keyring via `keyring-rs`.

---

## 12. Integration with nvim

The nvim side lives in our existing `lua/plugins/lang/dadbod/`:

- Add `duck-sqllsp` to `lspconfig`'s server list, with `cmd = { 'duck-sqllsp', 'server' }`.
- Replace the current `sqls` configuration block. Filetypes: sql, mysql, plsql.
- `initializationOptions` is populated from our `db_manager.store.state`
  via a small builder in `wiring.lua` -- the same connections users already
  manage with `:DBAdd`/`:DBSwitch` get pushed straight to duck-sqllsp.
- `:DBSwitch` sends `workspace/didChangeConfiguration` with
  `activeConnection` set; duck-sqllsp re-fetches the schema.
- The native `dsl-completion` items obviate our wrapped cmp source --
  `completion.lua` can shrink to a thin adapter that just sets cmp's
  filetype source list to `{ name = 'nvim_lsp' }` for SQL.
- `dsl-format` is the formatter; `conform.nvim` can drop sqlfluff/sql-formatter
  in favor of `lsp_format`, or duck-sqllsp can be exposed as a custom
  formatter spec.
- `:DBScope` sends a custom `workspace/executeCommand` so the server can
  narrow completion to a database or schema.
- Diagnostics flow naturally through `vim.diagnostic`; Trouble and
  todo-comments-nvim pick them up unchanged.

This is "drop-in" because we adhere to the LSP spec exactly. Nothing
neovim-specific is required server-side.

---

## 13. Performance targets

Numbers measured on a workspace of 200 `.sql` files totaling ~150KB.

| Metric                        | Target  |
|-------------------------------|---------|
| Cold startup to ready         | < 80ms |
| Hot parse of a 5KB file       | < 2ms   |
| Completion P50                | < 8ms   |
| Completion P99                | < 25ms  |
| Hover P50                     | < 4ms   |
| Diagnostics after full edit   | < 30ms  |
| Schema introspect on 100 tables, 1000 columns | < 250ms |
| Memory steady-state on 200 file workspace | < 60MB |

Salsa memoization plus rowan's structural sharing keep edits cheap.
Async I/O for schema fetches.

---

## 14. Testing

- **Snapshot diagnostics**: `tests/diagnostics/sql001/*.sql` ->
  `*.expected.txt`. Run via `insta` for golden files.
- **Parser fuzz**: `cargo-fuzz` target on `dsl-parser::parse(&str)` to
  guarantee no panic.
- **Round-trip**: every parsed file round-trips through the CST back to
  text without losing any byte.
- **End-to-end LSP**: `dsl-test` spins up the server over a duplex pipe,
  drives it with `lsp-test`, asserts request/response shapes.
- **Integration with a real Postgres**: docker-compose spins up a temp
  cluster with a known schema. CI runs the introspection + completion
  end-to-end.
- **Property tests**: `proptest` generates random valid SQL within a
  bounded grammar, verifies the parser accepts and prints back equivalent
  syntax.

CI matrix: stable + nightly, linux/macos. Coverage tracked via
`cargo llvm-cov`.

---

## 15. Milestones

**v0.1 -- "speaks LSP" (1 week of focused work, slashed because we don't write a parser)**
- `dsl-knowledge` with keyword / function / type tables.
- `dsl-parse` Postgres path via `sqlparser` (postgres dialect).
- `dsl-server` initialize / shutdown, capabilities advertised over stdio.
- `dsl-cli` with `server` subcommand.
- `textDocument/completion` for keywords + types + functions.
- `textDocument/hover` for keywords + types + functions.

**v0.2 -- "schema-aware" (1.5 weeks)**
- `dsl-conn` Postgres driver via `sqlx::postgres`.
- `dsl-catalog` cache + XDG persistence.
- `dsl-resolve` name resolution.
- Completion returns tables, columns, schemas from the live catalog.
- Hover renders table column lists, column types, parents.
- Diagnostics sql000 (parser), sql001/sql002/sql003 (unresolved family).
- `pg_query` opt-in via `dsl-parse` feature flag for full PG fidelity.

**v0.3 -- "linter" (2 weeks)**
- Remaining 23 diagnostics (sql004 .. sql027) excluding type-inference rules.
- Code actions for the auto-fixable subset.
- `textDocument/formatting` via `dsl-format` (shells out to existing pipeline).
- `textDocument/signatureHelp`.

**v0.4 -- "navigation" (1 week)**
- definition, references, rename, document/workspace symbols.
- Semantic tokens.

**v0.5 -- "MySQL + SQLite" (1 week)**
- `sqlx::mysql` and `sqlx::sqlite` drivers in `dsl-conn`.
- `sqlparser` mysql + sqlite dialects in `dsl-parse`.
- Cross-dialect snapshot tests.

**v0.6 -- "types + polish"**
- Lightweight type-inference module (replacing the v0.1 string types).
- inlayHint for parameter and result types.
- Performance tuning to hit v1 targets.
- Public 1.0 release.

---

## 16. Risk register

| Risk | Mitigation |
|------|------------|
| Postgres grammar is enormous; parser becomes a tar pit. | Cover the 90% common syntax first; treat exotic stuff (advisory locks, custom operators, GIN expressions) as ERROR nodes that don't break analysis. |
| Schema introspection holds a long-lived DB connection. | Pool with timeouts; cached catalog allows the server to function with no live DB. |
| Salsa learning curve for contributors. | Vendor a tiny example in `dsl-test/examples/salsa101.rs`; doc-comment every query function. |
| Diagnostics false positives erode trust. | Each rule ships with `tests/diagnostics/sql<NNN>/cases.sql` containing 5+ positive and 5+ negative cases. Gate releases on a 0-false-positive run against a curated corpus. |
| Catalog drift between editor cache and live DB. | TTL + manual refresh command. Show staleness badge in editor via custom LSP notification. |
| Auth / secrets in config files. | Env-var indirection, keyring backend, password fields elided from diagnostics. |

---

## 17. Open questions

- Adopt `tower-lsp-server` (active fork) or stay on `tower-lsp` (original, less active). Tentative answer: tower-lsp-server.
- Use `salsa-2024` or `salsa-3.0` (preview). Tentative: salsa-2024 for stability.
- Ship a single `duck-sqllsp` binary that also doubles as `dsl-cli`, or two binaries? Tentative: one binary with subcommands.
- License: MIT or Apache-2.0? Match the existing duck-* projects' MIT.

---

## 18. Glossary

- **CST** -- concrete syntax tree, every byte preserved.
- **AST** -- typed projection of the CST; what analysis reads.
- **HIR** -- high-level IR; AST with names resolved to catalog ids.
- **Catalog** -- the cached database schema (tables, columns, functions).
- **Dialect** -- the SQL flavour we're parsing (postgres / mysql / sqlite).
- **LSP capability** -- a feature the server advertises in initialize.
- **Rule** -- a single diagnostic check in `dsl-analysis`.

---

## 19. References

- `sqlparser-rs`: <https://github.com/apache/datafusion-sqlparser-rs>
- `pg_query.rs`: <https://github.com/pganalyze/pg_query.rs>
- libpg_query: <https://github.com/pganalyze/libpg_query>
- `sqlx`: <https://github.com/launchbadge/sqlx>
- `tower-lsp-server`: <https://github.com/tower-lsp-community/tower-lsp-server>
- `tower-lsp`: <https://github.com/ebkalderon/tower-lsp>
- LSP spec 3.18: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/>
- Postgres docs root: <https://www.postgresql.org/docs/current/>
- nvim built-in LSP client: `:help lsp`
