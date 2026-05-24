// Connection store: persists `.duck-sqllsp.toml` at the workspace root.
// Format mirrors what the LSP's `dsl-server::config` parser reads from
// the same file -- one [[connections]] table per saved spec.
//
// Connections from a sibling Neovim setup are also surfaced (read-only
// import) so users who already configured connections in nvim's
// dadbod/db_manager flow see them automatically in the VS Code sidebar.

import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { workspace } from "vscode";

export type ConnectionKind = "postgres" | "mysql" | "sqlite";

export interface ConnectionSpec {
  name: string;
  kind: ConnectionKind;
  url: string;
  /// "toml" when sourced from .duck-sqllsp.toml in the workspace,
  /// "nvim" when imported from the nvim db_manager store on disk.
  source?: "toml" | "nvim";
}

export interface ProjectConfig {
  active?: string;
  connections: ConnectionSpec[];
}

function configPath(): string | undefined {
  const folder = workspace.workspaceFolders?.[0];
  if (!folder) return undefined;
  return path.join(folder.uri.fsPath, ".duck-sqllsp.toml");
}

export function loadConfig(): ProjectConfig {
  const file = configPath();
  let cfg: ProjectConfig = { connections: [] };
  if (file && fs.existsSync(file)) {
    cfg = parseToml(fs.readFileSync(file, "utf8"));
    cfg.connections.forEach((c) => (c.source = "toml"));
  }
  // Pull in nvim's db_manager store too -- users who set things up
  // through nvim's dadbod flow should see those entries without
  // re-typing. Merge by name; existing toml entries win.
  for (const n of loadNvimConnections()) {
    if (!cfg.connections.some((c) => c.name === n.name)) {
      cfg.connections.push(n);
    }
  }
  const active = loadNvimActive();
  if (!cfg.active && active) {
    cfg.active = active;
  }
  return cfg;
}

/// Pull connection list from ~/.local/share/nvim/db_connections.json
/// (the format the user's nvim dadbod/db_manager flow writes). Each
/// entry has `name`, `driver`, `user`, `host`, `port`, `database`,
/// `password`. Converted to a libpq-style URL so it matches the
/// dsl-conn spec the LSP expects.
function loadNvimConnections(): ConnectionSpec[] {
  const file = path.join(os.homedir(), ".local", "share", "nvim", "db_connections.json");
  if (!fs.existsSync(file)) return [];
  try {
    const raw = JSON.parse(fs.readFileSync(file, "utf8"));
    if (!Array.isArray(raw)) return [];
    return raw.map((c: any): ConnectionSpec | undefined => {
      const driver = String(c.driver ?? "postgresql").toLowerCase();
      const kind: ConnectionKind | undefined =
        driver.startsWith("postgres") ? "postgres"
        : driver.startsWith("mysql") ? "mysql"
        : driver.startsWith("sqlite") ? "sqlite"
        : undefined;
      if (!kind) return undefined;
      const user = c.user ?? "";
      const pass = c.password ? `:${encodeURIComponent(c.password)}` : "";
      const auth = user ? `${encodeURIComponent(user)}${pass}@` : "";
      const host = c.host ?? "localhost";
      const port = c.port ? `:${c.port}` : "";
      const db = c.database ?? "";
      const url = kind === "sqlite"
        ? `sqlite://${c.database ?? ""}`
        : `${kind}://${auth}${host}${port}/${db}`;
      return { name: c.name ?? "unnamed", kind, url, source: "nvim" };
    }).filter((c): c is ConnectionSpec => !!c);
  } catch {
    return [];
  }
}

function loadNvimActive(): string | undefined {
  const file = path.join(os.homedir(), ".local", "share", "nvim", "db_active.json");
  if (!fs.existsSync(file)) return undefined;
  try {
    const obj = JSON.parse(fs.readFileSync(file, "utf8"));
    if (typeof obj === "object" && obj && typeof obj.active === "string") {
      return obj.active;
    }
  } catch {}
  return undefined;
}

export function saveConfig(cfg: ProjectConfig): string | undefined {
  const file = configPath();
  if (!file) return undefined;
  // Only persist toml-sourced entries -- nvim-sourced ones live in
  // their own store and should not be duplicated.
  const persistable: ProjectConfig = {
    active: cfg.active,
    connections: cfg.connections.filter((c) => c.source !== "nvim"),
  };
  fs.writeFileSync(file, serializeToml(persistable), "utf8");
  return file;
}

// Tiny TOML reader. Only handles the shape this extension writes; if
// the user hand-edits more exotic TOML we leave it alone via the
// fallback parse path (best effort).
function parseToml(text: string): ProjectConfig {
  const cfg: ProjectConfig = { connections: [] };
  let current: Record<string, string> | null = null;
  let inConnections = false;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    if (line === "[[duck_sqllsp.connections]]" || line === "[[connections]]") {
      if (current) cfg.connections.push(specFromBag(current));
      current = {};
      inConnections = true;
      continue;
    }
    if (line.startsWith("[")) {
      if (current) cfg.connections.push(specFromBag(current));
      current = null;
      inConnections = false;
      continue;
    }
    const eq = line.indexOf("=");
    if (eq < 0) continue;
    const key = line.slice(0, eq).trim();
    let val = line.slice(eq + 1).trim();
    if (val.startsWith('"') && val.endsWith('"')) val = val.slice(1, -1);
    if (inConnections && current) {
      current[key] = val;
    } else if (key === "active_connection" || key === "active") {
      cfg.active = val;
    }
  }
  if (current) cfg.connections.push(specFromBag(current));
  return cfg;
}

function specFromBag(b: Record<string, string>): ConnectionSpec {
  return {
    name: b.name ?? "default",
    kind: (b.kind as ConnectionKind) ?? "postgres",
    url: b.url ?? "",
  };
}

function serializeToml(cfg: ProjectConfig): string {
  const lines: string[] = [];
  lines.push("# duck-sqllsp project config (managed by the VS Code extension).");
  lines.push("");
  if (cfg.active) {
    lines.push(`[duck_sqllsp]`);
    lines.push(`active_connection = "${escape(cfg.active)}"`);
    lines.push("");
  }
  for (const c of cfg.connections) {
    lines.push("[[duck_sqllsp.connections]]");
    lines.push(`name = "${escape(c.name)}"`);
    lines.push(`kind = "${escape(c.kind)}"`);
    lines.push(`url  = "${escape(c.url)}"`);
    lines.push("");
  }
  return lines.join("\n");
}

function escape(s: string): string {
  return s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}
