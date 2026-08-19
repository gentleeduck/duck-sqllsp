# Configuration reference

Every setting duck-sqllsp reads, with its default and what it actually
changes. If a key isn't listed here, the server doesn't read it.

## Where config comes from

Three sources, applied in this order — later wins:

1. **Editor `initializationOptions`**, sent at startup (VS Code settings,
   nvim `settings = { ... }`).
2. **`.duck-sqllsp.toml`** or **`.duck-sqllsp.json`**, found by walking up
   from the workspace root — and, for editors that don't send a root, from
   the first file you open.
3. **`workspace/didChangeConfiguration`**, if your editor pushes updates
   while the server is running.

The project file wins over editor settings deliberately, so a repository
can pin its own formatting without every contributor configuring their
editor.

## Both spellings work

Every key accepts `camelCase` and `snake_case`, and the top-level table
accepts `duckSqllsp` or `duck_sqllsp`:

```toml
[duck_sqllsp.style]
keywordCase = "lower"    # same as keyword_case
```

The wrapper table is optional. These are equivalent:

```toml
[duck_sqllsp.style.formatter]     [style.formatter]
singleLine = true                 singleLine = true
```

> Wrapped configs that set only `style` or only `rules` — with no
> connection — were silently ignored before v0.1.1, as were all bare
> `.duck-sqllsp.toml` files. If you wrote config that appeared to do
> nothing, this was why.

## Top level

| Key | Type | Default | Effect |
|---|---|---|---|
| `activeConnection` | string | none | Name of the entry in `connections` to introspect against. |
| `connections` | array of tables | empty | See [Connections](#connections). |
| `dialect` | enum | from connection, else `postgresql` | `postgresql` (`postgres`, `pg`), `mysql` (`mariadb`), `sqlite` (`sqlite3`), `mssql` (`sqlserver`, `tsql`, `transactsql`). Sets the parser, the completion vocabulary, and the formatter language. |
| `requireConnection` | bool | `true` | When true, catalog-dependent diagnostics (`sql001` unresolved table, `sql002` unknown column) stay quiet with no active connection. Set `false` to see them against the offline catalog. |
| `scope` | string | none | Reserved for scoping introspection; not yet used. |
| `rules` | table | empty | See [Per-rule severity](#per-rule-severity). |
| `style` | table | see below | See [Style](#style). |

## Connections

```toml
[[duck_sqllsp.connections]]
name = "local"
url  = "postgres://user:pass@localhost:5432/mydb"
```

The driver is inferred from the URL scheme, so it must be present:

| Scheme | Driver |
|---|---|
| `postgres://`, `postgresql://` | postgres |
| `mysql://`, `mariadb://` | mysql |
| `sqlite://`, `sqlite:` | sqlite |

Anything else resolves to `unknown` and won't connect — a bare filesystem
path is not enough for SQLite, write `sqlite:./dev.db`.

duck-sqllsp works fully offline: with no connection it builds a catalog by
scanning every `*.sql` file in the workspace, so completion, hover, and
most diagnostics still work against tables you have only written down.

## Per-rule severity

Keys are diagnostic codes; values are `error`, `warning`, `info`, `hint`,
or `off` (`ignore` and `none` are accepted as synonyms for `off`).

```toml
[duck_sqllsp.rules]
sql015 = "off"        # silence `= NULL`
sql001 = "hint"       # downgrade unresolved-table
```

`duck-sqllsp rules` lists every code with its title and default severity.

## Style

Identifier casing applied to completion items as you accept them.

| Key | Type | Default | Effect |
|---|---|---|---|
| `keywordCase` | `upper` / `lower` / `preserve` | `upper` | Casing for SQL keywords. |
| `functionCase` | same | `lower` | Casing for function names. |
| `typeCase` | same | `upper` | Casing for type names. |
| `identifierCase` | same | `preserve` | Casing for tables, columns, schemas. `preserve` keeps whatever the database reports. |

### `style.createTable`

The DataGrip-style `CREATE TABLE` alignment pass, applied after the
external formatter.

| Key | Type | Default | Effect |
|---|---|---|---|
| `alignColumns` | bool | `true` | Pad column name / type / `NOT NULL` / `DEFAULT` into aligned columns. |
| `openParenOnNewLine` | bool | `true` | Put `(` on its own line after the table name. |
| `constraintsAtEnd` | bool | `true` | Move table-level constraints below the column list. |
| `columnGap` | integer | `4` | Spaces between aligned sub-columns. `1` is tight, `2`–`4` reads better. |
| `groupIndexes` | bool | `true` | Pack consecutive `CREATE INDEX` statements with no blank line between them. Only affects runs of two or more. |

### `style.formatter`

**Most of these are handed to the external
[`sql-formatter`](https://github.com/sql-formatter-org/sql-formatter)
CLI (v15+), and do nothing without it.** Install with
`npm i -g sql-formatter`; `duck-sqllsp doctor` tells you whether it was
found.

The **Works offline** column below says whether a setting still applies
when the binary is missing and only the built-in alignment pass runs:

| | |
|---|---|
| **yes** | handled by the built-in passes |
| **no** | needs `sql-formatter`; silently has no effect without it |

| Key | Type | Default | Works offline | Effect |
|---|---|---|---|---|
| `language` | string | `postgresql` | no | sql-formatter dialect. Left at the default, the open buffer's `dialect` drives it, so `mysql` backticks and `mssql` brackets tokenise correctly. |
| `tabWidth` | integer | `4` | yes | Indent width for the `CREATE TABLE` column body. Your editor's own tab setting overrides this per format request. The PL/pgSQL block indenter uses a fixed two spaces per level regardless. |
| `keywordCase` | `upper` / `lower` / `preserve` | `upper` | yes | Keyword casing. The built-in pass applies it to the `NOT NULL` / `DEFAULT` it re-emits; trailing constraints (`primary key`, `check (...)`) keep the case you wrote, since re-casing them would mean rewriting expressions. |
| `dataTypeCase` | same | `preserve` | no | Type-name casing. |
| `functionCase` | same | `lower` | no | Function-name casing. |
| `linesBetweenQueries` | integer | `1` | no | Blank lines between statements. |
| `expressionWidth` | integer | `80` | no | Column at which long projections, `WHERE` conjunctions, and `VALUES` lists wrap. |
| `denseOperators` | bool | `false` | no | Collapse spaces around `=`, `<>`, and friends. |
| `newlineBeforeSemicolon` | bool | `false` | no | Put the trailing `;` on its own line. |
| `logicalOperatorNewline` | `before` / `after` | `before` | no | Whether `AND` / `OR` lead the new line or trail the previous one. |
| `singleLine` | bool | `false` | yes | Collapse each DML statement onto one line. DDL is left alone so table layouts stay readable. |
| `compactClauses` | bool | `false` | yes | Middle ground: each top-level clause on its own line, but its body on the same line as the keyword. Ignored when `singleLine` is on. |

## A complete example

```toml
[duck_sqllsp]
activeConnection  = "local"
dialect           = "postgresql"
requireConnection = false

[duck_sqllsp.rules]
sql015 = "off"

[duck_sqllsp.style]
keywordCase    = "upper"
functionCase   = "lower"
typeCase       = "upper"
identifierCase = "preserve"

[duck_sqllsp.style.createTable]
alignColumns       = true
openParenOnNewLine = true
constraintsAtEnd   = true
columnGap          = 4
groupIndexes       = true

[duck_sqllsp.style.formatter]
language               = "postgresql"
tabWidth               = 2
expressionWidth        = 100
logicalOperatorNewline = "before"
denseOperators         = false
singleLine             = false
compactClauses         = false

[[duck_sqllsp.connections]]
name = "local"
url  = "postgres://user:pass@localhost:5432/mydb"
```

## Checking what took effect

Run the server with `RUST_LOG=info` and it logs the config file it loaded
on startup. In VS Code, **duck-sqllsp: Show Logs**. In nvim,
`:LspLog`.
