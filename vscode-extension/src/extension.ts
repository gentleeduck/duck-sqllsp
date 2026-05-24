import * as path from "path";
import * as vscode from "vscode";
import { ExtensionContext, commands, window, workspace } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import {
  ConnectionKind,
  ConnectionSpec,
  loadConfig,
  saveConfig,
} from "./connections";
import { ConnectionItem, ConnectionsProvider } from "./tree";

let client: LanguageClient | undefined;
let connectionsProvider: ConnectionsProvider | undefined;

export function activate(context: ExtensionContext) {
  connectionsProvider = new ConnectionsProvider();
  context.subscriptions.push(
    window.registerTreeDataProvider("duckSqllsp.connections", connectionsProvider),
    commands.registerCommand("duckSqllsp.addConnection", addConnection),
    commands.registerCommand("duckSqllsp.listConnections", listConnections),
    commands.registerCommand("duckSqllsp.setActiveConnection", setActiveConnection),
    commands.registerCommand("duckSqllsp.removeConnection", removeConnection),
    commands.registerCommand("duckSqllsp.refreshCatalog", refreshCatalog),
    commands.registerCommand("duckSqllsp.restartServer", restartServer),
    workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("duckSqllsp")) {
        connectionsProvider?.refresh();
      }
    }),
  );

  startClient(context);
}

function startClient(context: ExtensionContext) {
  const config = workspace.getConfiguration("duckSqllsp");
  const serverBin = config.get<string>("serverPath", "duck-sqllsp");
  const activeConnection = config.get<string>("activeConnection", "")
    || loadConfig().active
    || undefined;

  const serverOptions: ServerOptions = {
    run: {
      command: serverBin,
      args: ["server"],
      transport: TransportKind.stdio,
    },
    debug: {
      command: serverBin,
      args: ["server"],
      transport: TransportKind.stdio,
      options: {
        env: {
          ...process.env,
          RUST_LOG: "info,dsl_server::perf=debug",
        },
      },
    },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "sql" },
      { scheme: "file", language: "postgres" },
      { scheme: "untitled", language: "sql" },
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
    traceOutputChannel: window.createOutputChannel("duck-sqllsp trace"),
    outputChannel: window.createOutputChannel("duck-sqllsp"),
  };

  client = new LanguageClient(
    "duck-sqllsp",
    "duck-sqllsp",
    serverOptions,
    clientOptions,
  );

  context.subscriptions.push({ dispose: () => { void client?.stop(); } });
  void client.start();
}

// -------- Commands --------------------------------------------------

async function addConnection(): Promise<void> {
  const name = await window.showInputBox({
    prompt: "Connection name (used by `duckSqllsp.activeConnection`)",
    placeHolder: "local-dev",
    validateInput: (v) => (v && /^[A-Za-z0-9_-]+$/.test(v) ? null : "use letters, digits, _ or -"),
  });
  if (!name) return;

  const kind = (await window.showQuickPick(
    [
      { label: "postgres", description: "PostgreSQL via libpq URL" },
      { label: "mysql", description: "MySQL via DSN URL" },
      { label: "sqlite", description: "SQLite file path" },
    ],
    { placeHolder: "Database kind" },
  ))?.label as ConnectionKind | undefined;
  if (!kind) return;

  const placeholder =
    kind === "postgres"
      ? "postgres://user:password@host:5432/dbname"
      : kind === "mysql"
        ? "mysql://user:password@host:3306/dbname"
        : "sqlite:///absolute/path/to/file.db";
  const url = await window.showInputBox({
    prompt: `Connection URL (${kind})`,
    placeHolder: placeholder,
    password: false,
    ignoreFocusOut: true,
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
    Object.assign(existing, { kind, url });
  } else {
    cfg.connections.push({ name, kind, url });
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
      description: c.kind,
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
  // The server picks up config changes via workspace/didChangeConfiguration
  // -- nudge a restart so the new active connection takes effect.
  await restartServer();
}

async function restartServer(): Promise<void> {
  if (!client) return;
  await client.stop();
  void client.start();
  window.showInformationMessage("duck-sqllsp server restarted");
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) return undefined;
  return client.stop();
}
