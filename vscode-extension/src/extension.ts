import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { ExtensionContext, commands, window, workspace } from "vscode";
import {
  CloseAction,
  ErrorAction,
  LanguageClient,
  LanguageClientOptions,
  RevealOutputChannelOn,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import {
  ConnectionSpec,
  driverFromUrl,
  loadConfig,
  saveConfig,
} from "./connections";
import { SchemaProvider } from "./schema-tree";
import { ConnectionItem, ConnectionsProvider } from "./tree";

let client: LanguageClient | undefined;
let connectionsProvider: ConnectionsProvider | undefined;
let schemaProvider: SchemaProvider | undefined;
let statusItem: vscode.StatusBarItem | undefined;
let outputChannel: vscode.OutputChannel | undefined;
let traceChannel: vscode.OutputChannel | undefined;

export function activate(context: ExtensionContext) {
  // Output channels are the first thing we set up so every later
  // step can log into them. Any throw past this point is logged +
  // surfaced as a notification so the user can see what's failing.
  outputChannel = window.createOutputChannel("duck-sqllsp");
  traceChannel = window.createOutputChannel("duck-sqllsp trace");
  outputChannel.appendLine(`[ext] duck-sqllsp activating (extension version ${context.extension.packageJSON?.version ?? "?"})`);

  // CRITICAL: register every command FIRST, before anything that
  // might throw. If activation throws halfway, the commands are
  // already wired so the palette / sidebar buttons don't error out
  // with "command not found". Each command body is wrapped in a
  // try/catch so a broken command doesn't poison the others.
  const safe =
    <A extends any[], R>(name: string, fn: (...args: A) => Promise<R> | R) =>
    async (...args: A): Promise<R | void> => {
      try {
        return await fn(...args);
      } catch (e: any) {
        outputChannel?.appendLine(`[cmd:${name}] error: ${e?.stack ?? e}`);
        window.showErrorMessage(`duck-sqllsp ${name}: ${e?.message ?? e}`);
      }
    };

  context.subscriptions.push(
    outputChannel,
    traceChannel,
    commands.registerCommand("duckSqllsp.addConnection", safe("addConnection", addConnection)),
    commands.registerCommand("duckSqllsp.listConnections", safe("listConnections", listConnections)),
    commands.registerCommand("duckSqllsp.setActiveConnection", safe("setActiveConnection", setActiveConnection)),
    commands.registerCommand("duckSqllsp.removeConnection", safe("removeConnection", removeConnection)),
    commands.registerCommand("duckSqllsp.refreshCatalog", safe("refreshCatalog", refreshCatalog)),
    commands.registerCommand("duckSqllsp.restartServer", safe("restartServer", restartServer)),
    commands.registerCommand("duckSqllsp.showLogs", () => outputChannel?.show(true)),
    commands.registerCommand("duckSqllsp.testConnection", safe("testConnection", testConnection)),
    commands.registerCommand("duckSqllsp.refreshSchema", () => schemaProvider?.refresh()),
    commands.registerCommand("duckSqllsp.insertColumnAtCursor", safe("insertColumnAtCursor", insertColumnAtCursor)),
    // The LSP's CodeLens handler emits these command IDs (note: dotted
    // form `duck-sqllsp.*`, not `duckSqllsp.*`, matching the server). The
    // extension implements them client-side so clicking `Run` / `EXPLAIN`
    // / `+ LIMIT 100` above a statement actually does something instead
    // of popping "command not found".
    commands.registerCommand("duck-sqllsp.runQuery", safe("runQuery", (sql: string) => runQuery(sql, "run"))),
    commands.registerCommand("duck-sqllsp.explainQuery", safe("explainQuery", (sql: string) => runQuery(sql, "explain"))),
    commands.registerCommand("duck-sqllsp.explainAnalyzeQuery", safe("explainAnalyzeQuery", (sql: string) => runQuery(sql, "explain-analyze"))),
    commands.registerCommand("duck-sqllsp.addLimit", safe("addLimit", addLimitToQuery)),
    commands.registerCommand("duck-sqllsp.noop", () => {}),
  );
  outputChannel.appendLine("[ext] commands registered");

  // Sidebar tree provider + status bar live after commands so a
  // failure here can't break the command surface.
  try {
    connectionsProvider = new ConnectionsProvider();
    schemaProvider = new SchemaProvider(() => client);
    context.subscriptions.push(
      window.registerTreeDataProvider("duckSqllsp.connections", connectionsProvider),
      window.registerTreeDataProvider("duckSqllsp.schema", schemaProvider),
      workspace.onDidChangeConfiguration((e) => {
        if (e.affectsConfiguration("duckSqllsp")) {
          connectionsProvider?.refresh();
          schemaProvider?.refresh();
        }
      }),
      // Auto-refresh the schema tree on every SQL buffer save so new
      // CREATE TABLE / FUNCTION / etc show up without manual refresh.
      workspace.onDidSaveTextDocument((doc) => {
        if (doc.languageId === "sql" || doc.fileName.endsWith(".sql")) {
          schemaProvider?.refresh();
        }
      }),
    );
  } catch (e: any) {
    outputChannel.appendLine(`[ext] connectionsProvider failed: ${e?.stack ?? e}`);
  }

  try {
    statusItem = window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    statusItem.text = "$(database) duck-sqllsp";
    statusItem.tooltip = "duck-sqllsp LSP status -- click to restart";
    statusItem.command = "duckSqllsp.restartServer";
    statusItem.show();
    context.subscriptions.push(statusItem);
  } catch (e: any) {
    outputChannel.appendLine(`[ext] statusItem failed: ${e?.stack ?? e}`);
  }

  startClient(context).catch((err) => {
    outputChannel?.appendLine(`[ext] failed to start client: ${err?.stack ?? err}`);
    window.showErrorMessage(`duck-sqllsp failed to start: ${err?.message ?? err}`);
    setStatus("error", "failed to start");
  });
}

async function startClient(context: ExtensionContext) {
  const config = workspace.getConfiguration("duckSqllsp");
  const configuredBin = config.get<string>("serverPath", "duck-sqllsp");
  const activeConnection = config.get<string>("activeConnection", "")
    || loadConfig().active
    || undefined;

  const serverBin = await resolveServerBin(configuredBin);
  if (!serverBin) {
    const msg = `duck-sqllsp binary not found. Tried PATH and \`${configuredBin}\`. Set \`duckSqllsp.serverPath\` to the full path.`;
    outputChannel?.appendLine(`[ext] ${msg}`);
    window.showErrorMessage(msg, "Show Logs").then((c) => { if (c === "Show Logs") outputChannel?.show(true); });
    setStatus("error", "binary missing");
    return;
  }
  outputChannel?.appendLine(`[ext] spawning: ${serverBin} server`);

  const serverOptions: ServerOptions = {
    run: {
      command: serverBin,
      args: ["server"],
      transport: TransportKind.stdio,
      options: {
        env: {
          ...process.env,
          RUST_LOG: process.env.RUST_LOG ?? "info",
        },
      },
    },
    debug: {
      command: serverBin,
      args: ["server"],
      transport: TransportKind.stdio,
      options: {
        env: {
          ...process.env,
          RUST_LOG: "debug,dsl_server::perf=debug",
        },
      },
    },
  };

  const clientOptions: LanguageClientOptions = {
    // Be generous with the selector so completion / hover fire on any
    // SQL-ish buffer the editor recognises -- some installations use
    // `postgres`, `plsql`, or even just `sql`.
    documentSelector: [
      { scheme: "file", language: "sql" },
      { scheme: "file", language: "postgres" },
      { scheme: "file", language: "plpgsql" },
      { scheme: "file", language: "plsql" },
      { scheme: "untitled", language: "sql" },
      { scheme: "file", pattern: "**/*.sql" },
      { scheme: "file", pattern: "**/*.pgsql" },
      { scheme: "file", pattern: "**/*.psql" },
    ],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/{.duck-sqllsp.toml,.duck-sqllsp.json}"),
      configurationSection: "duckSqllsp",
    },
    initializationOptions: {
      duckSqllsp: {
        activeConnection,
      },
    },
    traceOutputChannel: traceChannel,
    outputChannel: outputChannel,
    // Surface the output channel on any LSP error so users see what
    // went wrong without having to dig through Output dropdowns.
    revealOutputChannelOn: RevealOutputChannelOn.Error,
    errorHandler: {
      error: (err, msg, count) => {
        outputChannel?.appendLine(`[client] error #${count}: ${err.message}`);
        setStatus("error", err.message);
        return { action: ErrorAction.Continue };
      },
      closed: () => {
        outputChannel?.appendLine("[client] connection closed");
        setStatus("error", "server closed");
        return { action: CloseAction.DoNotRestart };
      },
    },
  };

  client = new LanguageClient(
    "duck-sqllsp",
    "duck-sqllsp",
    serverOptions,
    clientOptions,
  );

  context.subscriptions.push({ dispose: () => { void client?.stop(); } });
  setStatus("starting", "starting...");
  try {
    await client.start();
    setStatus("ready", activeConnection ? `connected: ${activeConnection}` : "offline mode");
    outputChannel?.appendLine("[ext] client started");
  } catch (err: any) {
    setStatus("error", err?.message ?? "start failed");
    outputChannel?.appendLine(`[ext] client.start() rejected: ${err?.stack ?? err}`);
    window.showErrorMessage(`duck-sqllsp could not connect: ${err?.message ?? err}`);
  }
}

/// Try to locate the duck-sqllsp binary.
///   * Absolute path -> check it exists.
///   * Bare name -> probe ~/.local/bin, ~/.cargo/bin, /usr/local/bin,
///     /usr/bin, and finally trust the configured name (PATH-resolved
///     at spawn time).
async function resolveServerBin(configured: string): Promise<string | undefined> {
  // Expand `~` and `${env:VAR}`.
  const expand = (p: string) =>
    p.replace(/^~(?=$|\/|\\)/, process.env.HOME ?? "~")
      .replace(/\$\{env:([A-Za-z_][A-Za-z_0-9]*)\}/g, (_, k) => process.env[k] ?? "");
  const candidate = expand(configured);
  if (path.isAbsolute(candidate)) {
    return fs.existsSync(candidate) ? candidate : undefined;
  }
  // Already on PATH? Try a quick `which`-style probe.
  const probe = [
    path.join(process.env.HOME ?? "", ".local", "bin", candidate),
    path.join(process.env.HOME ?? "", ".cargo", "bin", candidate),
    "/usr/local/bin/" + candidate,
    "/opt/homebrew/bin/" + candidate,
    "/usr/bin/" + candidate,
  ];
  for (const p of probe) {
    if (fs.existsSync(p)) {
      outputChannel?.appendLine(`[ext] resolved bare \`${configured}\` -> ${p}`);
      return p;
    }
  }
  // Fall back to the bare name; OS PATH lookup might still succeed.
  return candidate;
}

function setStatus(state: "ready" | "starting" | "error", detail: string) {
  if (!statusItem) return;
  const prefix = state === "ready" ? "$(database)" : state === "starting" ? "$(sync~spin)" : "$(error)";
  statusItem.text = `${prefix} duck-sqllsp`;
  statusItem.tooltip = `duck-sqllsp -- ${detail}\nClick to restart server`;
}

// -------- Commands --------------------------------------------------

async function addConnection(): Promise<void> {
  const name = await window.showInputBox({
    prompt: "Connection name (used by `duckSqllsp.activeConnection`)",
    placeHolder: "local-dev",
    validateInput: (v) => (v && /^[A-Za-z0-9_-]+$/.test(v) ? null : "use letters, digits, _ or -"),
  });
  if (!name) return;

  const url = await window.showInputBox({
    prompt: "Connection URL",
    placeHolder: "postgres://user:password@host:5432/dbname  |  mysql://...  |  sqlite:///path/to/file.db",
    password: false,
    ignoreFocusOut: true,
    validateInput: (v) => {
      if (!v) return "URL is required";
      if (!/^(postgres|postgresql|mysql|mariadb|sqlite):/i.test(v)) {
        return "URL must start with postgres:// / postgresql:// / mysql:// / mariadb:// / sqlite:";
      }
      return null;
    },
  });
  if (!url) return;

  const cfg = loadConfig();
  const existing = cfg.connections.find((c) => c.name === name);
  if (existing) {
    const overwrite = await window.showWarningMessage(
      `Connection \`${name}\` already exists. Overwrite?`,
      { modal: true },
      "Overwrite",
    );
    if (overwrite !== "Overwrite") return;
    existing.url = url;
  } else {
    cfg.connections.push({ name, url });
  }
  if (!cfg.active) cfg.active = name;

  const file = saveConfig(cfg);
  if (file) {
    window.showInformationMessage(`Saved connection \`${name}\` -> ${file}`);
    connectionsProvider?.refresh();
    await refreshCatalog();
  } else {
    window.showErrorMessage("No workspace folder open -- cannot save .duck-sqllsp.toml");
  }
}

async function listConnections(): Promise<void> {
  const cfg = loadConfig();
  if (cfg.connections.length === 0) {
    window.showInformationMessage("No saved connections. Run `duck-sqllsp: Add Connection`.");
    return;
  }
  const pick = await window.showQuickPick(
    cfg.connections.map((c) => ({
      label: c.name + (c.name === cfg.active ? " (active)" : ""),
      description: driverFromUrl(c.url),
      detail: c.url,
      conn: c,
    })),
    { placeHolder: "Select to make active" },
  );
  if (pick) await setActive(pick.conn.name);
}

async function setActiveConnection(item?: ConnectionItem): Promise<void> {
  const cfg = loadConfig();
  let name = item?.spec.name;
  if (!name) {
    name = (
      await window.showQuickPick(
        cfg.connections.map((c) => c.name),
        { placeHolder: "Pick connection to activate" },
      )
    ) || undefined;
  }
  if (!name) return;
  await setActive(name);
}

async function setActive(name: string): Promise<void> {
  const cfg = loadConfig();
  cfg.active = name;
  saveConfig(cfg);
  connectionsProvider?.refresh();
  await workspace.getConfiguration("duckSqllsp").update("activeConnection", name);
  window.showInformationMessage(`duck-sqllsp active connection: ${name}`);
  await refreshCatalog();
}

async function removeConnection(item?: ConnectionItem): Promise<void> {
  const cfg = loadConfig();
  let name = item?.spec.name;
  if (!name) {
    name = (
      await window.showQuickPick(
        cfg.connections.map((c) => c.name),
        { placeHolder: "Pick connection to remove" },
      )
    ) || undefined;
  }
  if (!name) return;
  const confirm = await window.showWarningMessage(
    `Remove connection \`${name}\`?`,
    { modal: true },
    "Remove",
  );
  if (confirm !== "Remove") return;
  cfg.connections = cfg.connections.filter((c) => c.name !== name);
  if (cfg.active === name) delete cfg.active;
  saveConfig(cfg);
  connectionsProvider?.refresh();
  window.showInformationMessage(`Removed connection \`${name}\``);
}

async function refreshCatalog(): Promise<void> {
  await restartServer();
}

/// Asks the LSP to ping the active connection and reports the result.
/// The server-side handler is `workspace/executeCommand` with the
/// command `duck-sqllsp.testConnection`.
async function testConnection(): Promise<void> {
  if (!client) {
    window.showErrorMessage("duck-sqllsp not running");
    return;
  }
  try {
    const res = (await client.sendRequest("workspace/executeCommand", {
      command: "duck-sqllsp.testConnection",
      arguments: [],
    })) as { ok: boolean; name: string; message: string; tables: number } | null;
    if (!res) {
      window.showWarningMessage("duck-sqllsp: server did not respond to testConnection");
      return;
    }
    const headline = res.ok
      ? `duck-sqllsp: ${res.name} OK -- ${res.message}`
      : `duck-sqllsp: ${res.name} failed -- ${res.message}`;
    if (res.ok) {
      window.showInformationMessage(headline);
    } else {
      window.showErrorMessage(headline, "Show Logs").then((c) => {
        if (c === "Show Logs") outputChannel?.show(true);
      });
    }
  } catch (e: any) {
    window.showErrorMessage(`duck-sqllsp: testConnection failed -- ${e?.message ?? e}`);
  }
}

async function insertColumnAtCursor(name: string): Promise<void> {
  const ed = window.activeTextEditor;
  if (!ed) {
    window.showWarningMessage("duck-sqllsp: open a SQL file before inserting a column.");
    return;
  }
  await ed.edit((eb) => {
    for (const sel of ed.selections) {
      eb.replace(sel, name);
    }
  });
}

/// `Run` / `EXPLAIN` / `EXPLAIN ANALYZE` CodeLens handler. The LSP itself
/// doesn't execute SQL, so we route the query to a terminal running the
/// active connection's CLI client (psql / mysql / sqlite3). Falls back to
/// copying the SQL to the clipboard when no active connection or no
/// suitable CLI is configured.
type RunMode = "run" | "explain" | "explain-analyze";
async function runQuery(sql: string, mode: RunMode): Promise<void> {
  if (!sql || !sql.trim()) {
    window.showWarningMessage("duck-sqllsp: empty query");
    return;
  }
  const stmt = decorateForMode(sql, mode);
  const cfg = loadConfig();
  const active = cfg.connections.find((c) => c.name === cfg.active) ?? undefined;
  if (!active) {
    await vscode.env.clipboard.writeText(stmt);
    window.showInformationMessage(
      "duck-sqllsp: no active connection -- statement copied to clipboard.",
    );
    return;
  }
  const cli = cliFor(active.url);
  if (!cli) {
    await vscode.env.clipboard.writeText(stmt);
    window.showInformationMessage(
      `duck-sqllsp: no CLI mapping for \`${active.url}\` -- statement copied to clipboard.`,
    );
    return;
  }
  const term = window.terminals.find((t) => t.name === "duck-sqllsp") ?? window.createTerminal("duck-sqllsp");
  term.show(true);
  const escaped = stmt.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  term.sendText(`${cli.binary} ${cli.urlArg(active.url)} -c "${escaped}"`, true);
}

function decorateForMode(sql: string, mode: RunMode): string {
  const trimmed = sql.trim().replace(/;\s*$/, "");
  switch (mode) {
    case "run":
      return trimmed + ";";
    case "explain":
      return `EXPLAIN ${trimmed};`;
    case "explain-analyze":
      return `EXPLAIN ANALYZE ${trimmed};`;
  }
}

function cliFor(url: string): { binary: string; urlArg: (u: string) => string } | undefined {
  if (/^postgres(?:ql)?:/i.test(url)) {
    return { binary: "psql", urlArg: (u) => JSON.stringify(u) };
  }
  if (/^mysql:/i.test(url) || /^mariadb:/i.test(url)) {
    return { binary: "mysql", urlArg: (u) => `--defaults-extra-file=<(echo "[client]") ${JSON.stringify(u)}` };
  }
  if (/^sqlite:/i.test(url)) {
    const path = url.replace(/^sqlite:(?:\/+)?/i, "");
    return { binary: "sqlite3", urlArg: (_) => JSON.stringify(path) };
  }
  return undefined;
}

/// `+ LIMIT 100` CodeLens handler. The LSP doesn't tell us where in the
/// document the statement lives -- the lens only forwards the SQL text
/// and the limit. Search the active buffer for that exact text, append
/// `LIMIT <n>` before the trailing semicolon (or at end-of-statement if
/// no semicolon), and replace in place. Falls back to a snippet at the
/// cursor when the text isn't found.
async function addLimitToQuery(sql: string, n: number): Promise<void> {
  const ed = window.activeTextEditor;
  if (!ed) {
    window.showWarningMessage("duck-sqllsp: open the SQL file first.");
    return;
  }
  const doc = ed.document;
  const haystack = doc.getText();
  const idx = haystack.indexOf(sql);
  const limited = appendLimit(sql, n);
  if (idx < 0) {
    // Fallback: insert at cursor.
    await ed.edit((eb) => {
      for (const sel of ed.selections) {
        eb.insert(sel.active, `LIMIT ${n}`);
      }
    });
    return;
  }
  const start = doc.positionAt(idx);
  const end = doc.positionAt(idx + sql.length);
  await ed.edit((eb) => {
    eb.replace(new vscode.Range(start, end), limited);
  });
}

function appendLimit(sql: string, n: number): string {
  const trimmed = sql.replace(/\s*;\s*$/, "");
  return `${trimmed}\nLIMIT ${n};`;
}

async function restartServer(): Promise<void> {
  if (!client) return;
  setStatus("starting", "restarting...");
  await client.stop();
  try {
    await client.start();
    setStatus("ready", "restarted");
    window.showInformationMessage("duck-sqllsp server restarted");
  } catch (err: any) {
    setStatus("error", err?.message ?? "restart failed");
    window.showErrorMessage(`duck-sqllsp restart failed: ${err?.message ?? err}`);
  }
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) return undefined;
  return client.stop();
}
