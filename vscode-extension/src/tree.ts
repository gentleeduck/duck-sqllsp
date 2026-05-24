// Sidebar tree view: lists saved connections with the active one
// marked. Items expose context actions through the package.json
// `view/item/context` menu wiring.

import * as vscode from "vscode";
import { ConnectionSpec, ProjectConfig, driverFromUrl, loadConfig } from "./connections";

export class ConnectionsProvider implements vscode.TreeDataProvider<ConnectionItem> {
  private _onDidChange = new vscode.EventEmitter<ConnectionItem | undefined | void>();
  readonly onDidChangeTreeData = this._onDidChange.event;

  refresh(): void {
    this._onDidChange.fire();
  }

  getTreeItem(el: ConnectionItem): vscode.TreeItem {
    return el;
  }

  getChildren(): Thenable<ConnectionItem[]> {
    const cfg: ProjectConfig = loadConfig();
    const items = cfg.connections.map((c) => new ConnectionItem(c, c.name === cfg.active));
    if (items.length === 0) {
      const empty = new ConnectionItem({ name: "(no connections saved)", url: "" }, false);
      empty.contextValue = "empty";
      empty.command = { command: "duckSqllsp.addConnection", title: "Add" };
      return Promise.resolve([empty]);
    }
    return Promise.resolve(items);
  }
}

export class ConnectionItem extends vscode.TreeItem {
  constructor(
    public readonly spec: ConnectionSpec,
    public readonly active: boolean,
  ) {
    super(spec.name, vscode.TreeItemCollapsibleState.None);
    this.tooltip = spec.url || "(no URL)";
    this.description = driverFromUrl(spec.url);
    this.contextValue = spec.url ? "connection" : "empty";
    this.iconPath = new vscode.ThemeIcon(
      active ? "pass-filled" : "plug",
      active ? new vscode.ThemeColor("charts.green") : undefined,
    );
  }
}
