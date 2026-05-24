# Changelog

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
