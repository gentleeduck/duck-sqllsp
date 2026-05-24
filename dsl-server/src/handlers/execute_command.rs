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

pub const SUPPORTED: &[&str] = &["duck-sqllsp.testConnection"];

pub async fn run(state: &ServerState, params: ExecuteCommandParams) -> Option<Value> {
  let _g = crate::handlers::perf::Guard::new("execute_command");
  match params.command.as_str() {
    "duck-sqllsp.testConnection" => Some(test_connection(state, &params.arguments).await),
    _ => None,
  }
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
