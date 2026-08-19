# Security Policy

## Supported Versions

Pre-1.0. Only `0.1.x` is supported. Security fixes go into the
latest minor.

| Version | Supported |
| --- | --- |
| 0.1.x   | yes |
| < 0.1   | no  |

## Reporting a Vulnerability

Do not open a public issue for security vulnerabilities.

Email [ahmedayobbusiness@gmail.com](mailto:ahmedayobbusiness@gmail.com)
with:

- a description of the issue
- the affected version (`duck-sqllsp version`)
- a minimal reproducer if possible
- your assessment of the impact

We aim to acknowledge within 72 hours and release a fix or mitigation
within 30 days for high severity issues.

## Threat surfaces

duck-sqllsp is a language server. It reads SQL you are editing, reads
`.sql` files in your workspace, and — when you configure a connection —
talks to a database. The relevant surfaces:

- **Connection credentials.** `.duck-sqllsp.toml` holds database URLs,
  and a URL usually holds a password. The file is read from the project
  directory, so it is easy to commit one by accident. Keep credentials
  in a file you have gitignored, or in editor-level settings rather than
  the project file. duck-sqllsp never transmits a connection string
  anywhere except to the database it names.

- **Introspection queries.** With an active connection the server runs
  read-only catalog queries (`pg_catalog`, `information_schema`, or the
  driver equivalent) on its own schedule — at startup, on save, and on
  an explicit refresh. Point it at a role with no more than read access
  to the schema you want completed. It does not need, and should not be
  given, write privileges.

- **Code lenses that execute SQL.** The `Run`, `EXPLAIN`, and
  `EXPLAIN ANALYZE` lenses spawn a terminal running `psql`, `mysql`, or
  `sqlite3` against the active connection with the statement under the
  cursor. That statement executes for real, against a real database, and
  `EXPLAIN ANALYZE` executes the query rather than just planning it.
  These lenses are offered only to VS Code and its forks, and only on
  an explicit click.

- **The external formatter.** Formatting shells out to a `sql-formatter`
  binary found on `PATH` (or in the nvim/mason, `~/.local/bin`, and asdf
  shim directories). Whatever binary answers to that name is executed
  with the buffer contents on stdin. On a machine with a writable
  directory early in `PATH`, that is a hijack opportunity — the same one
  every tool with an optional external dependency has. The formatter is
  optional; without it, formatting falls back to a built-in pass.

- **Workspace file reads.** The offline catalog is built by walking the
  workspace root for `*.sql` files (bounded at 5000 files and 4 MiB
  each, skipping hidden directories, `node_modules`, `target`, and
  similar). If the workspace root resolves higher than you expect, that
  walk covers more of your filesystem than you expect —
  `duck-sqllsp doctor` prints the root it derived.

- **SQL is analysed, never executed.** The lint engine, completion, and
  hover are all static: they parse and inspect text. Rules that reason
  about dynamic SQL (`EXECUTE`, `USING`, injection patterns) do so by
  reading the statement, not by running it. The only paths that reach a
  database are introspection and the code lenses above.

## Dependency policy

`cargo-deny` runs in CI over advisories, licenses, bans, and sources.
Ignored advisories are listed in [`deny.toml`](deny.toml), each with the
dependency path and the reason it cannot currently be fixed.
