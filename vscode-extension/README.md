# duck-sqllsp for VS Code

Thin VS Code client for [duck-sqllsp](https://github.com/gentleeduck/duck-sqllsp) -- a Postgres / MySQL / SQLite Language Server written in Rust.

## Upgrading from 0.1.0

If you see `command 'duckSqllsp.addConnection' not found` (or similar for any
other command), you have an older .vsix installed. Uninstall and reinstall:

```bash
code --uninstall-extension wildduck.duck-sqllsp-vscode
code --install-extension vscode-extension/duck-sqllsp-vscode-0.1.1.vsix
```

Then reload the VS Code window (`Developer: Reload Window` in the palette).

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

## Troubleshooting

Completion / hover not showing? Walk through these:

1. **Open the status bar entry** -- bottom right, `database` icon. Hover it for the current state (`starting…` / `connected: <name>` / `offline mode` / `error: …`). Click to restart.
2. **Open the Output panel and pick `duck-sqllsp`** from the dropdown. The activation log shows the exact command the extension spawned and any LSP error.
3. **Confirm the binary is on PATH**: `which duck-sqllsp` in a terminal. If not, set the absolute path in `duckSqllsp.serverPath` (User or Workspace settings).
4. **File must be recognised as SQL**: bottom-right language indicator should read `SQL`. `.sql`, `.pgsql`, `.psql` are auto-detected by the extension; for an untitled buffer choose "Change Language Mode" -> SQL.
5. **Without a DB connection** completion still works offline (tables / functions / sequences / types / roles harvested from the buffer plus default offline roles). Add a connection through the Connections sidebar entry for live introspection.
6. **Restart the server** with `Cmd/Ctrl+Shift+P` -> `duck-sqllsp: Restart Server` after editing `.duck-sqllsp.toml` or changing connection.

