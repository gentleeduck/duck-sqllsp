// Connection store: persists `.duck-sqllsp.toml` at the workspace root.
// Single source of truth: `{ name, url }`. The driver is inferred from
// the URL scheme by the server (`dsl-conn::ConnectionSpec::driver()`).

import * as fs from "fs";
import * as path from "path";
import { workspace } from "vscode";

export interface ConnectionSpec {
  name: string;
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
  return parseToml(fs.readFileSync(file, "utf8"));
}

export function saveConfig(cfg: ProjectConfig): string | undefined {
  const file = configPath();
  if (!file) return undefined;
  const previous = fs.existsSync(file) ? fs.readFileSync(file, "utf8") : "";
  fs.writeFileSync(file, mergeIntoExistingToml(previous, cfg), "utf8");
  return file;
}

/// Extract the driver from a URL scheme. Mirrors the server's
/// `ConnectionSpec::driver()` so the sidebar can show what kind of
/// DB each entry will connect to.
export function driverFromUrl(url: string): "postgres" | "mysql" | "sqlite" | "unknown" {
  const u = url.toLowerCase();
  if (u.startsWith("postgres://") || u.startsWith("postgresql://")) return "postgres";
  if (u.startsWith("mysql://") || u.startsWith("mariadb://")) return "mysql";
  if (u.startsWith("sqlite://") || u.startsWith("sqlite:")) return "sqlite";
  return "unknown";
}

/// Toml reader. Only handles the shape this extension writes.
/// Connection blocks accept both the new `url` field and the older
/// host/port/user/password/database (transparently converted to a
/// URL) so users upgrading from an earlier toml don't lose data.
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
  const name = b.name ?? "default";
  if (b.url) return { name, url: b.url };
  // Old shape: assemble a URL from the field-style entries.
  const driver = (b.driver ?? b.kind ?? "postgres").toLowerCase();
  const scheme = driver.startsWith("postgres") ? "postgres" : driver === "mariadb" ? "mysql" : driver;
  if (scheme === "sqlite") {
    return { name, url: `sqlite://${b.database ?? ""}` };
  }
  const user = b.user ? encodeURIComponent(b.user) : "";
  const pass = b.password ? `:${encodeURIComponent(b.password)}` : "";
  const auth = user ? `${user}${pass}@` : "";
  const host = b.host ?? "localhost";
  const port = b.port ? `:${b.port}` : "";
  const db = b.database ?? "";
  return { name, url: `${scheme}://${auth}${host}${port}/${db}` };
}

/// Splice cfg's connections + active_connection into the previous
/// .duck-sqllsp.toml content WITHOUT touching any other blocks
/// (style / formatter / createTable / comments / etc).
function mergeIntoExistingToml(previous: string, cfg: ProjectConfig): string {
  const lines = previous.split("\n");
  const out: string[] = [];
  let i = 0;

  let inMainBlock = false;
  let activeWritten = false;
  while (i < lines.length) {
    const line = lines[i];
    const trimmed = line.trim();
    if (trimmed === "[[duck_sqllsp.connections]]" || trimmed === "[[connections]]") {
      i++;
      while (i < lines.length && !lines[i].trim().startsWith("[")) i++;
      continue;
    }
    if (trimmed.startsWith("[duck_sqllsp]") || trimmed === "[duck_sqllsp]") {
      inMainBlock = true;
      out.push(line);
      i++;
      continue;
    }
    if (trimmed.startsWith("[") && trimmed !== "[duck_sqllsp]") {
      inMainBlock = false;
      out.push(line);
      i++;
      continue;
    }
    if (inMainBlock && (trimmed.startsWith("active_connection") || trimmed.startsWith("active "))) {
      if (cfg.active) {
        out.push(`active_connection = "${escape(cfg.active)}"`);
        activeWritten = true;
      }
      i++;
      continue;
    }
    out.push(line);
    i++;
  }

  if (cfg.active && !activeWritten) {
    const header = ["[duck_sqllsp]", `active_connection = "${escape(cfg.active)}"`, ""];
    let inj = 0;
    while (inj < out.length && (out[inj].startsWith("#") || out[inj].trim() === "")) inj++;
    out.splice(inj, 0, ...header);
  }

  while (out.length && out[out.length - 1].trim() === "") out.pop();

  if (cfg.connections.length > 0) out.push("");
  for (const c of cfg.connections) {
    out.push("[[duck_sqllsp.connections]]");
    out.push(`name = "${escape(c.name)}"`);
    out.push(`url  = "${escape(c.url)}"`);
    out.push("");
  }
  return out.join("\n");
}

function escape(s: string): string {
  return s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}
