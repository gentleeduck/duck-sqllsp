# Connect to a database (optional)

duck-sqllsp works **offline** -- completion / hover / diagnostics
all run off your buffer text + a built-in offline catalog. Adding a
live DB connection unlocks richer suggestions:

- Real table / column / function / sequence / extension lists from `information_schema`.
- Constraint info (PK, FK, CHECK) used by hover, code actions,
  and rule diagnostics like sql169 (unknown role).
- RLS policies + triggers + indexes from `pg_catalog`.

## Add one

Open the **duck-sqllsp** activity bar entry (database icon on the
left), then click the **+** in the Connections view title -- or run
`duck-sqllsp: Add Connection` from the command palette.

You'll be prompted for:
1. A name (used in settings to mark the active connection).
2. The connection URL (`postgres://` / `mysql://` / `mariadb://` /
   `sqlite:` -- the kind is inferred from the scheme, no separate
   prompt).

The entry persists to `.duck-sqllsp.toml` at the workspace root. The
LSP reads the same file -- shared with the CLI and other editors.
