<p align="center">
  <img src="./public/logo-dark.svg" alt="duck-sqllsp" width="120"/>
</p>

<h1 align="center">duck-sqllsp</h1>

<p align="center">
  Persistent SQL Language Server for PostgreSQL. tower-lsp + libpg_query. Built as a small set of focused crates so each piece is reusable on its own.
</p>

<p align="center">
  <a href="./LICENSE">MIT</a> -
  <a href="./CHANGELOG.md">Changelog</a> -
  <a href="./CONTRIBUTING.md">Contributing</a> -
  <a href="./dsl-cli">Crate docs</a> -
  <a href="./examples">Examples</a>
</p>

<p align="center">
  <a href="https://crates.io/crates/duck-sqllsp"><img src="https://img.shields.io/crates/v/duck-sqllsp.svg" alt="crates.io"/></a>
  <a href="https://docs.rs/duck-sqllsp"><img src="https://docs.rs/duck-sqllsp/badge.svg" alt="docs.rs"/></a>
  <a href="./LICENSE"><img src="https://img.shields.io/crates/l/duck-sqllsp.svg" alt="MIT"/></a>
</p>

---

## Install

```sh
cargo install duck-sqllsp
```

Or as a library:

```sh
cargo add dsl-analysis
```

## Quick start

```lua
-- nvim
vim.lsp.start({
  name = "duck-sqllsp",
  cmd = { "duck-sqllsp" },
  root_dir = vim.fn.getcwd(),
  filetypes = { "sql" },
  settings = {
    sql = {
      connection = "postgresql://user:pass@localhost:5432/mydb",
      diagnostics = { enabled = true },
      format = { align_columns = true, group_indexes = true },
    },
  },
})
```

```sh
duck-sqllsp run
```

## Workspace

| Crate | Role |
| --- | --- |
| [`dsl-parse`](dsl-parse) | SQL parser - libpg_query primary, sqlparser fallback |
| [`dsl-catalog`](dsl-catalog) | Schema catalog model - tables, columns, types, constraints, indexes |
| [`dsl-knowledge`](dsl-knowledge) | Static PG keyword / type / function reference with docs links |
| [`dsl-resolve`](dsl-resolve) | Name resolution, FROM/JOIN scope, CTE columns |
| [`dsl-format`](dsl-format) | SQL formatter - sql-formatter reflow + DataGrip-style alignment + PL/pgSQL body indent |
| [`dsl-analysis`](dsl-analysis) | Lint rule engine - 150+ PG-specific diagnostics with narrow ranges |
| [`dsl-completion`](dsl-completion) | Context-aware completion across 12 expression phases, alias + scope aware |
| [`dsl-hover`](dsl-hover) | Hover provider with cursor-side narrowing for dotted refs |
| [`dsl-conn`](dsl-conn) | Live PG connection layer for catalog introspection |
| [`dsl-server`](dsl-server) | tower-lsp server - definitions, diagnostics, format, semantic tokens |
| [`dsl-cli`](dsl-cli) | `duck-sqllsp` binary - stdio LSP, signal + parent-death handling |

## Examples

| Path | What it shows |
| --- | --- |
| [`examples/minimal`](examples/minimal) | Single buffer, no DB, diagnostics + completion + hover |
| [`examples/with-catalog`](examples/with-catalog) | Live PG connection - catalog introspection, FK-aware diagnostics |
| [`examples/format`](examples/format) | Round-trip CREATE TABLE / FUNCTION / TRIGGER / INDEX through the formatter |

## Build

```sh
cargo build --release
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Performance targets

| Metric | Target |
| --- | --- |
| Completion p50 latency | < 5 ms |
| Diagnostics p50 latency | < 20 ms |
| Hover p50 latency | < 3 ms |
| Format p50 latency | < 30 ms |
| Memory idle | < 30 MB |
| Memory @ 4 MiB file | < 150 MB |
| Cold start | < 50 ms |
| Document update | incremental, zero re-parse on cached handlers |

## Design

- **libpg_query** primary parser, **sqlparser** fallback. Statement-range tracked so every diagnostic has a precise byte range.
- **tower-lsp** for the protocol. Each handler is a thin shim over a pure-function crate.
- **Per-document parse cache** on a `OnceLock` - the first heavy handler after `didChange` pays the parse cost, the rest reuse it.
- **Space-preserving strip pattern** keeps 1:1 byte offsets when stripping strings/comments so diagnostic narrowing maps back to source exactly.
- **Catalog snapshots** are `parking_lot::RwLock` reads cloned before any `.await` - no guard crosses an await point.
- **`PR_SET_PDEATHSIG`** + SIGTERM / SIGINT / SIGHUP handling - the binary always dies with its editor.

JS / WASM plugin hooks are not in this workspace yet; they ship as separate crates once the core stabilises.

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
