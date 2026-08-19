# Editor setup

duck-sqllsp is a standard LSP server over stdio. Any client that can
launch a binary can use it.

```sh
duck-sqllsp server
```

`server` is the default, so a bare `duck-sqllsp` works too. `--stdio`,
`--node-ipc`, and `--socket=...` are accepted and ignored, because some
clients append them unconditionally.

**Root markers** — getting this right matters more than it looks: the
workspace root decides which `.sql` files are scanned into the offline
catalog, and therefore what completion knows about your schema. See
[troubleshooting](troubleshooting.md#no-completions-for-my-own-tables).

If your client sends no root at all, the server derives one by walking up
from the first file you open looking for `.duck-sqllsp.toml`,
`.duck-sqllsp.json`, `.git`, `Cargo.toml`, or `package.json` — and falls
back to that file's own directory. Configuring the root in your client is
better than relying on that.

**File types** — `.sql`, `.pgsql`, `.psql`.

---

## VS Code

Install `wildduck.duck-sqllsp-vscode`. Nothing else to configure.

It adds sidebar trees for connections and schema, and these commands:

| Command | What it does |
|---|---|
| Add Connection / Remove Connection | manage saved connections |
| Set Active Connection | choose which one introspection uses |
| Test Active Connection | check it answers |
| Refresh Catalog / Refresh Schema Tree | re-introspect |
| Restart Server | restart the language server |
| Show Logs | the server's output channel |
| Insert column at cursor | from the schema tree |

Run / EXPLAIN / EXPLAIN ANALYZE / + LIMIT 100 code lenses open a terminal
running `psql`, `mysql`, or `sqlite3` against the active connection.
Those lenses are shown only in VS Code and its forks — other clients have
no handler for the commands, so rather than offer a lens that errors on
click, the server omits them.

Settings:

| Setting | Default | Purpose |
|---|---|---|
| `duckSqllsp.serverPath` | `duck-sqllsp` | path to the binary, if it isn't on `PATH` |
| `duckSqllsp.activeConnection` | `""` | active connection name |
| `duckSqllsp.trace.server` | `off` | `messages` / `verbose` to log LSP traffic |

## Neovim 0.11+

```lua
vim.lsp.config('duck_sqllsp', {
  cmd = { 'duck-sqllsp', 'server' },
  filetypes = { 'sql', 'mysql', 'plsql' },
  root_markers = { '.duck-sqllsp.toml', '.duck-sqllsp.json', '.git' },
})
vim.lsp.enable('duck_sqllsp')
```

The server emits `$/progress`, so `vim.lsp.status()` and most statusline
plugins show "loading workspace..." while the `.sql` scan and any DB
introspection settle.

## Neovim 0.10 and earlier (nvim-lspconfig)

duck-sqllsp isn't in lspconfig's built-in registry, so register it:

```lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

if not configs.duck_sqllsp then
  configs.duck_sqllsp = {
    default_config = {
      cmd = { 'duck-sqllsp', 'server' },
      filetypes = { 'sql', 'mysql', 'plsql' },
      root_dir = lspconfig.util.root_pattern(
        '.duck-sqllsp.toml', '.duck-sqllsp.json', '.git'
      ),
      settings = {},
    },
  }
end

lspconfig.duck_sqllsp.setup({})
```

To pass configuration from Lua instead of a project file, use either key
— the server reads both, so it only matters when it arrives:

```lua
-- sent once at startup, as initializationOptions
init_options = {
  duckSqllsp = { requireConnection = false },
},
-- sent after startup, as workspace/didChangeConfiguration
settings = {
  duckSqllsp = { style = { keywordCase = 'lower' } },
},
```

A `.duck-sqllsp.toml` in the project wins over both, deliberately, so a
repository can pin its own formatting without every contributor
configuring their editor.

## Helix

In `languages.toml`:

```toml
[language-server.duck-sqllsp]
command = "duck-sqllsp"
args = ["server"]

[[language]]
name = "sql"
language-servers = ["duck-sqllsp"]
```

Check it registered with `hx --health sql`.

## Emacs (eglot)

```elisp
(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs
               '(sql-mode . ("duck-sqllsp" "server"))))
```

## Emacs (lsp-mode)

```elisp
(with-eval-after-load 'lsp-mode
  (lsp-register-client
   (make-lsp-client
    :new-connection (lsp-stdio-connection '("duck-sqllsp" "server"))
    :activation-fn (lsp-activate-on "sql")
    :server-id 'duck-sqllsp)))
```

## Sublime Text (LSP package)

In `LSP.sublime-settings`:

```json
{
  "clients": {
    "duck-sqllsp": {
      "enabled": true,
      "command": ["duck-sqllsp", "server"],
      "selector": "source.sql"
    }
  }
}
```

## Zed

Zed can only attach a language server to a language through an
extension — its `lsp` settings key configures servers that an extension
has already registered, so there is no settings-only snippet that will
work. No duck-sqllsp Zed extension exists yet.

## Any other client

The server needs nothing beyond stdio. A minimal client config is:

- **command**: `duck-sqllsp server`
- **languages**: `sql`
- **root**: nearest directory containing `.duck-sqllsp.toml` or `.git`

Configuration can arrive as `initializationOptions` at startup, via
`workspace/didChangeConfiguration`, or from a `.duck-sqllsp.toml` in the
project — see the [configuration reference](configuration.md).

---

## Checking it works

First, independently of any editor:

```sh
duck-sqllsp doctor
```

That reports the formatter binary, the config file it found, the
workspace root, how many `.sql` files the offline scan reached, and
connection health.

Then, in the editor:

1. Open a `.sql` file and type `SEL` — completion should offer `SELECT`.
2. Type `SELECT a FROM t WHERE x = NULL;` — a warning should appear on
   the `= NULL` (that's `sql015`).
3. Hover a keyword — a documentation card should appear.

If step 1 works but your own tables never appear, the workspace root is
almost certainly wrong; `doctor` prints what it resolved to.

If nothing happens at all, check the server is being launched: **VS
Code** → *duck-sqllsp: Show Logs*, **neovim** → `:LspLog`,
**Helix** → `hx --health sql`, **Emacs** → `M-x eglot-events-buffer`.

Set `RUST_LOG=debug` in the environment for more detail.
