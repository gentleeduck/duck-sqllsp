//! Async schema refresh.
//!
//! Given the current `ServerState`, find the active connection, build a
//! driver via `dsl-conn::build`, run introspection, and swap the catalog.
//! Errors are reported to the LSP client via `window/logMessage` so the
//! editor can show them without making the user check a log file.

use crate::state::ServerState;
use tower_lsp::Client;
use tower_lsp::lsp_types::{
  MessageType, NumberOrString, ProgressParams, ProgressParamsValue, WorkDoneProgress,
  WorkDoneProgressBegin, WorkDoneProgressEnd, WorkDoneProgressReport,
};

pub async fn refresh_catalog(state: ServerState, client: Client) {
  let cfg = state.config_snapshot();
  let active = match cfg.active() {
    Some(a) => a.clone(),
    None => {
      // Silent no-op when no connection is configured. Offline-mode
      // catalog comes from the workspace .sql scan (see state.rs);
      // showing a "no connection" message every time the user opens a
      // .sql file is noise when they intentionally don't want DB
      // introspection. To re-enable the message set duckSqllsp.config
      // explicitly.
      tracing::debug!("no active connection; catalog stays at workspace-derived");
      return;
    },
  };

  // Progress widget: editor shows a spinner while introspect runs.
  let token = NumberOrString::String(format!("duck-sqllsp-refresh-{}", active.name));
  send_progress(
    &client,
    &token,
    ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(WorkDoneProgressBegin {
      title: format!("duck-sqllsp: introspecting `{}`", active.name),
      cancellable: Some(false),
      message: Some("building driver...".into()),
      percentage: None,
    })),
  )
  .await;

  let driver = match dsl_conn::build(&active) {
    Ok(d) => d,
    Err(e) => {
      // Downgrade to WARNING; offline catalog still serves completion
      // and hover. Editors that surface ERROR as a popup were noisy on
      // every save when the DB was unreachable.
      client.log_message(MessageType::WARNING, format!("driver `{}` unavailable: {e}", active.name)).await;
      end_progress(&client, &token, Some(format!("driver `{}` unavailable", active.name))).await;
      return;
    },
  };

  send_progress(
    &client,
    &token,
    ProgressParamsValue::WorkDone(WorkDoneProgress::Report(WorkDoneProgressReport {
      cancellable: Some(false),
      message: Some("running introspect...".into()),
      percentage: None,
    })),
  )
  .await;

  match driver.introspect().await {
    Ok(cat) => {
      let tables = cat.tables().count();
      let cols: usize = cat.tables().map(|t| t.columns.len()).sum();
      let funcs = cat.functions.len();
      state.catalog.replace(cat);
      let msg = format!("schema loaded: {tables} tables / {cols} columns / {funcs} functions");
      client.log_message(MessageType::INFO, &msg).await;
      end_progress(&client, &token, Some(msg)).await;

      // Diagnostics that previously fired against an empty catalog
      // (sql001 unresolved table, sql002 unknown column) clear once
      // the live schema is known, so we re-run analysis on every
      // open buffer. This must happen here -- the language server
      // never gets a didChange to retrigger it.
      for (uri, _) in state.documents.snapshot() {
        crate::diagnostics::publish_for(&client, &state, &uri).await;
      }
    },
    Err(e) => {
      // WARNING (not ERROR) so editors don't pop a modal. The
      // workspace-derived catalog still serves completion / hover.
      client.log_message(MessageType::WARNING, format!("introspect on `{}` failed: {e}", active.name)).await;
      end_progress(&client, &token, Some(format!("introspect on `{}` failed", active.name))).await;
    },
  }
}

async fn send_progress(client: &Client, token: &NumberOrString, value: ProgressParamsValue) {
  let _ = client
    .send_notification::<tower_lsp::lsp_types::notification::Progress>(ProgressParams {
      token: token.clone(),
      value,
    })
    .await;
}

async fn end_progress(client: &Client, token: &NumberOrString, message: Option<String>) {
  send_progress(
    client,
    token,
    ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd { message })),
  )
  .await;
}
