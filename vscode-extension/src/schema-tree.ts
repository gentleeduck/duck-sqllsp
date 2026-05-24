// Schema sidebar tree: schemas -> tables -> columns. Pulls the
// catalog snapshot from the LSP via workspace/executeCommand.

import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/node";

interface CatalogSnapshot {
  schemas: SchemaDto[];
}

interface SchemaDto {
  name: string;
  tables: TableDto[];
}

interface TableDto {
  name: string;
  schema: string;
  kind: string;
  columns: ColumnDto[];
  constraintCount: number;
  indexCount: number;
  triggerCount: number;
}

interface ColumnDto {
  name: string;
  dataType: string;
  nullable: boolean;
  default: string | null;
}

type Node =
  | { kind: "schema"; schema: SchemaDto }
  | { kind: "table"; table: TableDto }
  | { kind: "column"; column: ColumnDto; table: TableDto };

export class SchemaProvider implements vscode.TreeDataProvider<Node> {
  private _onDidChange = new vscode.EventEmitter<Node | undefined | void>();
  readonly onDidChangeTreeData = this._onDidChange.event;
  private snapshot: CatalogSnapshot | undefined;

  constructor(private clientGetter: () => LanguageClient | undefined) {}

  refresh(): void {
    this.snapshot = undefined;
    this._onDidChange.fire();
  }

  async getChildren(node?: Node): Promise<Node[]> {
    if (!this.snapshot) {
      const client = this.clientGetter();
      if (!client) return [];
      try {
        const res = (await client.sendRequest("workspace/executeCommand", {
          command: "duck-sqllsp.getCatalog",
          arguments: [],
        })) as CatalogSnapshot | null;
        this.snapshot = res ?? { schemas: [] };
      } catch {
        return [];
      }
    }
    if (!node) {
      return this.snapshot.schemas.map((s) => ({ kind: "schema", schema: s }));
    }
    if (node.kind === "schema") {
      return node.schema.tables.map((t) => ({ kind: "table", table: t }));
    }
    if (node.kind === "table") {
      return node.table.columns.map((c) => ({ kind: "column", column: c, table: node.table }));
    }
    return [];
  }

  getTreeItem(n: Node): vscode.TreeItem {
    if (n.kind === "schema") {
      const item = new vscode.TreeItem(n.schema.name, vscode.TreeItemCollapsibleState.Collapsed);
      item.iconPath = new vscode.ThemeIcon("symbol-namespace");
      item.description = `${n.schema.tables.length} table(s)`;
      item.contextValue = "schema";
      return item;
    }
    if (n.kind === "table") {
      const item = new vscode.TreeItem(n.table.name, vscode.TreeItemCollapsibleState.Collapsed);
      const icon = n.table.kind === "view" ? "symbol-interface"
        : n.table.kind === "materializedview" ? "symbol-class"
        : "symbol-class";
      item.iconPath = new vscode.ThemeIcon(icon);
      const extras: string[] = [];
      if (n.table.constraintCount > 0) extras.push(`${n.table.constraintCount} cons`);
      if (n.table.indexCount > 0) extras.push(`${n.table.indexCount} idx`);
      if (n.table.triggerCount > 0) extras.push(`${n.table.triggerCount} trg`);
      item.description = `${n.table.columns.length} cols${extras.length ? " - " + extras.join(", ") : ""}`;
      item.tooltip = `${n.table.schema}.${n.table.name}`;
      item.contextValue = "table";
      return item;
    }
    const c = n.column;
    const item = new vscode.TreeItem(c.name, vscode.TreeItemCollapsibleState.None);
    item.iconPath = new vscode.ThemeIcon(c.nullable ? "symbol-field" : "symbol-key");
    item.description = c.dataType + (c.nullable ? "" : " NOT NULL");
    item.tooltip = `${n.table.schema}.${n.table.name}.${c.name}: ${c.dataType}${c.default ? ` DEFAULT ${c.default}` : ""}`;
    item.contextValue = "column";
    item.command = {
      command: "duckSqllsp.insertColumnAtCursor",
      title: "Insert",
      arguments: [c.name],
    };
    return item;
  }
}
