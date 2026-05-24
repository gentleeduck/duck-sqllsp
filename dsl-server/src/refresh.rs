//! Async schema refresh.
//!
//! Given the current `ServerState`, find the active connection, build a
//! driver via `dsl-conn::build`, run introspection, and swap the catalog.
//! Errors are reported to the LSP client via `window/logMessage` so the
//! editor can show them without making the user check a log file.

use crate::state::ServerState;
use tower_lsp::lsp_types::MessageType;
use tower_lsp::Client;

pub async fn refresh_catalog(state: ServerState, client: Client) {
    let cfg = state.config_snapshot();
    let active = match cfg.active() {
        Some(a) => a.clone(),
        None => {
            client
                .log_message(MessageType::INFO, "no active connection; catalog empty")
                .await;
            return;
        }
    };

    let driver = match dsl_conn::build(&active) {
        Ok(d) => d,
        Err(e) => {
            client
                .log_message(MessageType::ERROR, format!("driver build failed: {e}"))
                .await;
            return;
        }
    };

    match driver.introspect().await {
        Ok(cat) => {
            let tables = cat.tables().count();
            let cols: usize = cat.tables().map(|t| t.columns.len()).sum();
            let funcs = cat.functions.len();
            state.catalog.replace(cat);
            client
                .log_message(
                    MessageType::INFO,
                    format!("schema loaded: {tables} tables / {cols} columns / {funcs} functions"),
                )
                .await;

            // Diagnostics that previously fired against an empty catalog
            // (sql001 unresolved table, sql002 unknown column) clear once
            // the live schema is known, so we re-run analysis on every
            // open buffer. This must happen here -- the language server
            // never gets a didChange to retrigger it.
            for (uri, _) in state.documents.snapshot() {
                crate::diagnostics::publish_for(&client, &state, &uri).await;
            }
        }
        Err(e) => {
            client
                .log_message(MessageType::ERROR, format!("introspect failed: {e}"))
                .await;
        }
    }
}
