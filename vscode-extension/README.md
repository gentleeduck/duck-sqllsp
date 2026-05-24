# duck-sqllsp for VS Code

Thin VS Code client for [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp) -- a Postgres / MySQL / SQLite Language Server written in Rust.

## What you get

- Context-aware completion (tables, columns, functions, types, roles, GRANT privileges, ALTER TABLE sub-actions, snippet expansions).
- Hover docs for keywords, types, functions, tables, columns, sequences, extensions, RLS policies, triggers, indexes, roles.
- Inline column-name chips inside `INSERT ... VALUES (...)`.
- Inlay hints for `SELECT *` expansion and JOIN-without-ON predicate guesses.
- Workspace-wide find references / rename / document highlight.
- Go-to-definition + go-to-type-definition (cross-buffer).
- Folding ranges (BEGIN..END, paren bodies, multi-line block comments).
- On-type indentation.
- 150+ analysis rules (unknown table, unused alias, GROUP BY position, missing PK, ...).
- Document + workspace symbols.
- Code actions (extract subquery to CTE, EXISTS -> LATERAL, IN -> ANY, EXPLAIN ANALYZE wrap, ...).
- Document formatting via the bundled DataGrip-style aligner (plus `sql-formatter` when installed).

Works offline -- buffer-derived catalog covers tables / functions / sequences / types / extensions / roles. Connect a DB for richer suggestions.

## Install the server

Either:

- Cargo: `cargo install --git https://github.com/gentleeduck/duck-sqllsp duck-sqllsp` (then ensure `~/.cargo/bin` is on PATH).
- Manual: build the workspace and drop the `duck-sqllsp` binary somewhere on PATH.

The extension shells out to `duck-sqllsp server`. Adjust `duckSqllsp.serverPath` if the binary isn't on PATH.

## Configuration

| Setting | Default | Meaning |
|---|---|---|
| `duckSqllsp.serverPath` | `duck-sqllsp` | Path to the server binary. |
| `duckSqllsp.trace.server` | `off` | LSP traffic trace level (`off` / `messages` / `verbose`). |
| `duckSqllsp.activeConnection` | `""` | Named connection from `.duck-sqllsp.toml` to use for live introspection. Leave empty for offline-only mode. |

Project-level connections + style overrides live in `.duck-sqllsp.toml` (or `.duck-sqllsp.json`) at the workspace root.

## Build the extension locally

```bash
cd vscode-extension
npm install
npm run compile
# Press F5 in VS Code to launch an Extension Development Host.
```

To package a `.vsix`:

```bash
npm run package
```
