//! `workspace/executeCommand` handler.
//!
//! Supports a small set of duck-sqllsp-specific commands invoked by
//! editor extensions (currently the VS Code one). Returns a JSON
//! `Value` that the client deserialises into the shape it expects.
//!
//! Commands:
//!
//!   * `duck-sqllsp.testConnection [name?]`
//!       Tries to introspect the named connection (defaults to the
//!       currently-active one) and returns
//!       `{ ok: bool, name: string, message: string, tables: number }`.

use crate::state::ServerState;
use serde_json::{json, Value};
use tower_lsp::lsp_types::ExecuteCommandParams;

pub const SUPPORTED: &[&str] = &["duck-sqllsp.testConnection", "duck-sqllsp.getCatalog"];

pub async fn run(state: &ServerState, params: ExecuteCommandParams) -> Option<Value> {
  let _g = crate::handlers::perf::Guard::new("execute_command");
  match params.command.as_str() {
    "duck-sqllsp.testConnection" => Some(test_connection(state, &params.arguments).await),
    "duck-sqllsp.getCatalog" => Some(get_catalog(state)),
    _ => None,
  }
}

/// Snapshot of the merged catalog (live + workspace .sql scan + every
/// open buffer's source-derived tables) shaped for the VS Code schema
/// tree.
fn get_catalog(state: &ServerState) -> Value {
  let live = state.catalog.read().clone();
  let ws = state.workspace_offline_snapshot();
  let merged = dsl_completion::source_tables::merge(&live, &ws);
  let schemas: Vec<Value> = merged
    .schemas
    .iter()
    .map(|s| {
      let tables: Vec<Value> = s
        .tables
        .iter()
        .map(|t| {
          let cols: Vec<Value> = t
            .columns
            .iter()
            .map(|c| {
              json!({
                "name": c.name,
                "dataType": c.data_type,
                "nullable": c.nullable,
                "default": c.default,
              })
            })
            .collect();
          json!({
            "name": t.name,
            "schema": t.schema,
            "kind": format!("{:?}", t.kind).to_lowercase(),
            "columns": cols,
            "constraintCount": t.constraints.len(),
            "indexCount": t.indexes.len(),
            "triggerCount": t.triggers.len(),
          })
        })
        .collect();
      json!({ "name": s.name, "tables": tables })
    })
    .collect();
  json!({
    "schemas": schemas,
    "functionCount": merged.functions.len(),
    "sequenceCount": merged.sequences.len(),
    "extensionCount": merged.extensions.len(),
    "roleCount": merged.roles.len(),
  })
}

async fn test_connection(state: &ServerState, args: &[Value]) -> Value {
  let cfg = state.config_snapshot();
  // Optional first arg: explicit connection name. Fall back to the
  // active connection from the config.
  let name = args.first().and_then(|v| v.as_str()).map(|s| s.to_string());
  let spec = match name.as_deref() {
    Some(n) => cfg.connections.iter().find(|c| c.name == n).cloned(),
    None => cfg.active().cloned(),
  };
  let Some(spec) = spec else {
    return json!({
      "ok": false,
      "name": name.unwrap_or_default(),
      "message": "no matching connection configured",
      "tables": 0,
    });
  };
  let label = spec.name.clone();
  let driver = match dsl_conn::build(&spec) {
    Ok(d) => d,
    Err(e) => {
      return json!({
        "ok": false,
        "name": label,
        "message": format!("driver build failed: {e}"),
        "tables": 0,
      });
    }
  };
  match driver.introspect().await {
    Ok(cat) => {
      let tables: usize = cat.tables().count();
      json!({
        "ok": true,
        "name": label,
        "message": format!("connected; introspected {tables} table(s)"),
        "tables": tables,
      })
    }
    Err(e) => json!({
      "ok": false,
      "name": label,
      "message": format!("introspection failed: {e}"),
      "tables": 0,
    }),
  }
}
