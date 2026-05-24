# Changelog

## 0.1.6

- Schema sidebar tree. Second view under the duck-sqllsp activity bar
  entry shows schemas -> tables -> columns of the merged offline
  catalog (every CREATE TABLE in the workspace + live DB intro when
  connected). Click a column to insert its name at the cursor.
- Server: workspace/executeCommand gains `duck-sqllsp.getCatalog`
  -- returns the merged catalog as JSON for the sidebar to render.

## 0.1.5

- URL-only connection schema. Server-side `dsl_conn::ConnectionSpec` is
  now `{ name, url }`; driver inferred from URL scheme
  (postgres://, postgresql://, mysql://, mariadb://, sqlite:). Old
  host/port/user/password/database fields removed. Extension's
  Add Connection prompts for name + URL and writes the same shape.
- Toml writer preserves every other block (style, formatter,
  createTable, comments) when adding/removing connections. The
  full project config that lived in `.duck-sqllsp.toml` before is
  never silently dropped on save.
- Toml reader accepts both the new `url` field and the legacy
  field-style spec (host/port/user/password/database) so existing
  configs auto-upgrade on first load -- the next save rewrites them
  as URLs.

## 0.1.4

- Drop nvim store import. The VS Code extension now reads only its
  own `.duck-sqllsp.toml`. No cross-editor coupling -- nvim users keep
  their dadbod/db_manager flow, VS Code users keep their sidebar +
  toml, and they share data only when the user explicitly puts an
  entry in `.duck-sqllsp.toml`.
- Tree item description simplified back to `<kind>` (no
  `- toml`/`- nvim` source tag).

## 0.1.3

**Critical bug fix.** 0.1.0..0.1.2 shipped without bundled runtime
dependencies -- `.vscodeignore` excluded `node_modules/**` so
`require("vscode-languageclient/node")` failed at extension load
time, before `activate()` ever ran. Net effect: zero commands
registered, every palette invocation returned "command not found".

This release bundles the production node_modules (~1MB) so the
extension actually loads. Future work: bundle via esbuild to keep
the .vsix small.

## 0.1.2

- Activation hardening: commands are registered as the very first
  thing in activate() now, before anything that could throw. Each
  command body is wrapped in a try/catch that logs to the
  duck-sqllsp output channel and shows the message as a toast --
  so a broken command can no longer mask others as "command not
  found".
- ConnectionsProvider and the status bar item moved into separate
  try/catch blocks for the same reason.
- Output channel logs the extension version on every activation so
  users can confirm which build is running.

## 0.1.1

- Activation: re-added `onCommand:*` events for every contributed command so
  palette / inline-icon invocations activate the extension when it wasn't
  already running. Fixes "command 'duckSqllsp.addConnection' not found" and
  the same message for every other command.
- Activation: added `onView:duckSqllsp.connections` so the sidebar's
  database view activates the extension on first open. Fixes "There is no
  data provider registered that can provide view data".
- Sidebar: every command now ships an icon (`$(add)`, `$(refresh)`,
  `$(trash)`, `$(check)`, `$(zap)`, `$(output)`, `$(debug-restart)`). The
  view-title and inline view-item menus render them as toolbar buttons.
- Tree items: active connection rendered with a `pass-filled` green icon;
  inactive ones with `plug`. Description tag (`postgres - nvim`,
  `postgres - toml`) tells you where each entry came from.
- Nvim import: the extension now reads
  `~/.local/share/nvim/db_connections.json` and `db_active.json` -- the
  files the nvim dadbod/db_manager flow writes -- and surfaces those
  connections in the sidebar (read-only). Active connection from nvim is
  honoured. Existing setups Just Work without re-entering credentials.
- Test Connection command (`$(zap)` icon): asks the LSP to ping the active
  connection and shows the result as a toast.
- Walkthrough: 3-step Welcome panel walkthrough (install binary / connect
  DB / try it) added under Welcome -> Walkthroughs.
- Binary discovery: when `duckSqllsp.serverPath` is the bare name
  `duck-sqllsp`, the extension probes `~/.local/bin`, `~/.cargo/bin`,
  `/usr/local/bin`, `/opt/homebrew/bin`, `/usr/bin` and uses the first
  hit. Absolute paths honoured verbatim. `~` expanded.
- Status bar (bottom right) shows server state: starting / connected:
  <name> / offline mode / error: <message>. Click to restart.

## 0.1.0

- First release. LSP client over stdio + connection-management sidebar +
  12 SQL snippets.
