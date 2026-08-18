# Troubleshooting

## Start here

```sh
duck-sqllsp doctor
```

It reports what the server can actually see — the formatter binary, which
config file was found and whether it set anything, where the workspace
root landed, how many `.sql` files the offline scan reached, and whether
the connection answers. Run it from your project directory, or pass a
path.

Exit status is `1` if something is broken and `0` if everything works,
so it's safe to put in CI. Warnings don't fail: running with no database
connection is a normal way to use this server.

Its output is the most useful thing to attach to a bug report.

---

## Formatting barely changes anything

`doctor` will say:

```
[warn] sql-formatter          not found on PATH
```

The heavy lifting — statement reflow, keyword casing, expression
wrapping — is done by the external [`sql-formatter`](https://github.com/sql-formatter-org/sql-formatter)
CLI. Without it, formatting still runs, but only the built-in
`CREATE TABLE` alignment pass, so output changes far less than you'd
expect and nothing tells you why.

```sh
npm i -g sql-formatter
```

The binary is also picked up from `~/.local/share/nvim/mason/bin`,
`~/.local/bin`, and `~/.asdf/shims`.

## My config isn't doing anything

First check `doctor` found the file at all:

```
[ok  ] config file            /path/to/.duck-sqllsp.toml
[warn] config contents        parsed, but set nothing recognisable
```

That second line means the file parsed but every key fell back to its
default — almost always a key name that doesn't exist. **Unknown keys are
ignored silently**, so a typo is invisible. Check names against the
[configuration reference](configuration.md).

Both `camelCase` and `snake_case` are accepted, and the `[duck_sqllsp]`
wrapper is optional.

> Before v0.1.1 there was a bug here: a config that set only `style` or
> only `rules`, with no connection, was discarded outright, as was every
> bare `.duck-sqllsp.toml`. If you're on an older build, add a
> `connections` entry or upgrade.

## No completions for my own tables

The server builds an offline catalog by scanning `.sql` files under the
workspace root, so this is usually a root problem. `doctor` shows both:

```
[ok  ] workspace root         /home/me/project
[ok  ] offline catalog        14 .sql file(s), 37 table(s), 4 function(s)
```

- **0 files** — the root is wrong, or your schema lives outside it.
- **files but 0 tables** — they were read but no `CREATE TABLE` was
  derived. Check for a syntax error near the top of the file.
- **root at `/`** — reported as a failure. No marker file
  (`.duck-sqllsp.toml`, `.git`, `Cargo.toml`, `package.json`) was found
  above your directory, so the scan would try to walk the whole
  filesystem. Add one at your project root.

Editors that don't send a workspace root at startup make the server
derive one from the first file you open, which is when this usually
goes wrong. In neovim, set `root_markers`.

## Unresolved-table and unknown-column warnings never appear

By design. `sql001` and `sql002` need a real schema to be sure a name is
wrong, so with no active connection they stay quiet rather than flagging
every table you haven't created yet.

To run them against the offline catalog instead:

```toml
[duck_sqllsp]
requireConnection = false
```

## Diagnostics I don't want

Silence any rule by code — `duck-sqllsp rules` lists them all:

```toml
[duck_sqllsp.rules]
sql015 = "off"       # off / ignore / none
sql001 = "hint"      # or downgrade instead of silencing
```

## The connection won't connect

```
[FAIL] connection             prod -- unrecognised URL scheme
```

The driver comes from the URL scheme, so it has to be there:
`postgres://`, `postgresql://`, `mysql://`, `mariadb://`, `sqlite://`,
`sqlite:`. A bare filesystem path is not enough for SQLite — write
`sqlite:./dev.db`.

If the scheme is right and it still fails, `doctor` prints the driver's
own error. The server falls back to the offline catalog when introspection
fails, so completion keeps working from your `.sql` files.

## Nothing works at all

Check the server is actually being started:

```sh
duck-sqllsp version
```

Then look at the LSP log — **VS Code**: *duck-sqllsp: Show Logs*.
**neovim**: `:LspLog`. Start the server with `RUST_LOG=debug` for more.

If the process starts and exits immediately, run `duck-sqllsp server`
directly in a terminal: it will wait on stdin, which is correct. It
should not print anything or exit.

## Large files feel slow

Documents over 4 MiB skip the heavy handlers (completion, hover, semantic
tokens, code lenses, inlay hints) rather than block the editor. Syntax
highlighting from your editor still works; the server just stops
contributing. This is a fixed limit, not configurable.

## Reporting a bug

Include:

1. `duck-sqllsp doctor` output.
2. `duck-sqllsp version`.
3. Editor and version.
4. The smallest SQL that reproduces it.

If it's a wrong or missing diagnostic, `duck-sqllsp lint file.sql` shows
the same analysis without the editor in the loop, which separates a rule
bug from an integration problem.
