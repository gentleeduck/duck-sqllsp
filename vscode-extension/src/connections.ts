// Connection store: persists `.duck-sqllsp.toml` at the workspace root.
// Format mirrors what the LSP's `dsl-server::config` parser reads from
// the same file -- one [[connections]] table per saved spec.

import * as fs from "fs";
import * as path from "path";
import { workspace } from "vscode";

export type ConnectionKind = "postgres" | "mysql" | "sqlite";

export interface ConnectionSpec {
  name: string;
  kind: ConnectionKind;
  url: string;
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
  if (!file || !fs.existsSync(file)) {
    return { connections: [] };
  }
  const text = fs.readFileSync(file, "utf8");
  return parseToml(text);
}

export function saveConfig(cfg: ProjectConfig): string | undefined {
  const file = configPath();
  if (!file) return undefined;
  fs.writeFileSync(file, serializeToml(cfg), "utf8");
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
