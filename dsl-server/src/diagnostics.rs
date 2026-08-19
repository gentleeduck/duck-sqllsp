//! Diagnostics: one analysis pass, two delivery channels.
//!
//! [`compute`] is the single source of truth -- it runs the analysis
//! engine over the current document and maps the results into LSP
//! `Diagnostic`s, applying the per-rule severity overrides from
//! `.duck-sqllsp.toml`.
//!
//! On top of it sit the two LSP delivery models:
//!
//!   * **Push** ([`publish_for`]) -- `textDocument/publishDiagnostics`,
//!     driven by us from `did_open` / `did_change` / catalog refresh.
//!   * **Pull** (`handlers/diagnostic.rs`) -- the client asks via
//!     `textDocument/diagnostic` on its own cadence (LSP 3.17).
//!
//! Exactly one is active per session: clients that advertise pull
//! support render *both* channels, so pushing to a pulling client shows
//! every diagnostic twice. [`publish_for`] no-ops in pull mode, and
//! [`invalidate_all`] picks the right way to say "re-run analysis"
//! after a catalog swap.

use crate::state::ServerState;
use ropey::Rope;
use text_size::TextRange;
use tower_lsp::Client;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range, Url};

/// Opaque token identifying one diagnostic result for `uri`. Two calls
/// return the same id exactly when the analysis inputs are unchanged
/// (document bytes + everything covered by
/// [`ServerState::analysis_generation`]), which lets the pull handler
/// answer with an `Unchanged` report instead of re-running the engine.
pub fn result_id(state: &ServerState, uri: &Url) -> Option<String> {
  use std::hash::{Hash, Hasher};
  let doc = state.documents.get(uri)?;
  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  doc.text.hash(&mut hasher);
  doc.dialect.hash(&mut hasher);
  Some(format!("{:x}.{}", hasher.finish(), state.analysis_generation()))
}

/// Run the analysis engine over `uri` and map the findings to LSP
/// diagnostics. Returns `None` when the document is gone or a newer
/// `didChange` superseded it mid-analysis (the caller should drop the
/// result rather than ship stale data).
///
/// Sync and client-free on purpose, so both the push and the pull path
/// can call it.
pub fn compute(state: &ServerState, uri: &Url) -> Option<(Vec<Diagnostic>, i32)> {
  let doc = state.documents.get(uri)?;
  let snapshot_version = doc.version;
  let cache = doc.parsed();
  // Must run before `doc.text` / `doc.rope` are moved out below --
  // `derived_catalog()` borrows the whole `Document`.
  let derived = doc.derived_catalog();
  let text = doc.text;
  let rope = doc.rope;

  // Offline-mode enrichment: merge live catalog with text-scanned
  // sequences / types / extensions / functions / roles + AST-derived
  // tables so analysis rules still see something useful when the DB
  // isn't connected. Cloned rather than held so no parking_lot guard
  // outlives the analysis run (the push caller awaits afterwards, and
  // the guard is not Send).
  let live = state.catalog.read().clone();
  let ws_offline = state.workspace_offline_snapshot();
  let cat = dsl_completion::source_tables::merge(&dsl_completion::source_tables::merge(&live, &derived), &ws_offline);
  let doc_dialect = state.documents.get(uri).map(|d| d.dialect).unwrap_or(dsl_parse::Dialect::Postgres);
  let raw = dsl_analysis::run_with_dialect(&text, &cache.file, &cache.scopes, &cat, doc_dialect);

  // Cancellation check #1: skip mapping work if a newer didChange
  // already arrived. The next publish_for call will produce diagnostics
  // for the fresher buffer.
  if state.documents.is_stale(uri, snapshot_version) {
    tracing::debug!(uri = %uri, "diagnostics dropped: doc version superseded mid-analysis");
    return None;
  }

  // Per-rule severity overrides from .duck-sqllsp.toml.
  let cfg = state.config_snapshot();
  let diagnostics = raw
    .into_iter()
    .filter_map(|d| {
      let sev = if let Some(over) = cfg.rules.get(d.code) {
        match over.to_ascii_lowercase().as_str() {
          "off" | "ignore" | "none" => return None,
          "error" => DiagnosticSeverity::ERROR,
          "warning" | "warn" => DiagnosticSeverity::WARNING,
          "info" | "information" => DiagnosticSeverity::INFORMATION,
          "hint" => DiagnosticSeverity::HINT,
          _ => map_severity(d.severity),
        }
      } else {
        map_severity(d.severity)
      };
      Some(Diagnostic {
        range: to_lsp_range(&rope, d.range),
        severity: Some(sev),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(d.code.to_string())),
        source: Some("duck-sqllsp".into()),
        message: d.message,
        ..Default::default()
      })
    })
    .collect::<Vec<_>>();

  // Cancellation check #2: right before we hand the result back.
  if state.documents.is_stale(uri, snapshot_version) {
    tracing::debug!(uri = %uri, "diagnostics dropped: doc version superseded before publish");
    return None;
  }

  Some((diagnostics, snapshot_version))
}

/// Push channel: analyse `uri` and send `textDocument/publishDiagnostics`.
///
/// No-op when the client pulls -- see the module docs.
pub async fn publish_for(client: &Client, state: &ServerState, uri: &Url) {
  if state.client_pulls_diagnostics() {
    return;
  }
  let Some((diagnostics, version)) = compute(state, uri) else { return };
  client.publish_diagnostics(uri.clone(), diagnostics, Some(version)).await;
}

/// Retract every diagnostic previously published for `uri`.
///
/// The server owns the lifetime of pushed diagnostics: the client holds
/// whatever we last sent until we send something else. Close a buffer
/// without this and its findings sit in the Problems panel forever,
/// pointing at a file the user is no longer editing -- and clicking one
/// reopens the file just to show a stale complaint.
///
/// Pull-mode clients discard a document's report when they close it, so
/// there is nothing to retract.
pub async fn clear_for(client: &Client, state: &ServerState, uri: &Url) {
  if state.client_pulls_diagnostics() {
    return;
  }
  client.publish_diagnostics(uri.clone(), Vec::new(), None).await;
}

/// Tell the client that previously-delivered diagnostics are stale for
/// *every* open document. Used after a catalog swap, which can clear a
/// sql001 unresolved-table finding without the buffer changing at all.
///
/// Push clients get a fresh publish per document; pull clients get one
/// `workspace/diagnostic/refresh` and re-pull on their own schedule.
pub async fn invalidate_all(client: &Client, state: &ServerState) {
  if state.client_pulls_diagnostics() {
    // Ignored by clients without `refreshSupport`; they will re-pull on
    // the next edit anyway.
    let _ = client.workspace_diagnostic_refresh().await;
    return;
  }
  for (uri, _) in state.documents.snapshot() {
    publish_for(client, state, &uri).await;
  }
}

fn map_severity(s: dsl_analysis::Severity) -> DiagnosticSeverity {
  match s {
    dsl_analysis::Severity::Error => DiagnosticSeverity::ERROR,
    dsl_analysis::Severity::Warning => DiagnosticSeverity::WARNING,
    dsl_analysis::Severity::Info => DiagnosticSeverity::INFORMATION,
    dsl_analysis::Severity::Hint => DiagnosticSeverity::HINT,
  }
}

fn to_lsp_range(rope: &Rope, range: TextRange) -> Range {
  let start: u32 = range.start().into();
  let end: u32 = range.end().into();
  Range {
    start: byte_to_position(rope, start as usize),
    end: byte_to_position(rope, (end as usize).min(rope.len_bytes())),
  }
}

fn byte_to_position(rope: &Rope, byte: usize) -> Position {
  let byte = byte.min(rope.len_bytes());
  let line_idx = rope.byte_to_line(byte);
  let line_start_byte = rope.line_to_byte(line_idx);
  let line_slice = rope.line(line_idx);
  let bytes_in_line = byte.saturating_sub(line_start_byte);
  // Walk the line counting utf-16 code units per char up to the byte.
  let mut char_utf16 = 0u32;
  let mut bytes_seen = 0usize;
  for c in line_slice.chars() {
    if bytes_seen >= bytes_in_line {
      break;
    }
    char_utf16 += c.len_utf16() as u32;
    bytes_seen += c.len_utf8();
  }
  Position { line: line_idx as u32, character: char_utf16 }
}
