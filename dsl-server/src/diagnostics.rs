//! Publish diagnostics to the LSP client.
//!
//! Runs the analysis engine on the current document's text and pushes
//! the result through `textDocument/publishDiagnostics`. Called from
//! `did_open`, `did_change`, and after a successful catalog refresh
//! (the catalog change can flip an unresolved-table diagnostic).

use crate::state::ServerState;
use ropey::Rope;
use text_size::TextRange;
use tower_lsp::Client;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range, Url};

pub async fn publish_for(client: &Client, state: &ServerState, uri: &Url) {
  let Some(doc) = state.documents.get(uri) else {
    return;
  };
  let snapshot_version = doc.version;
  let cache = doc.parsed();
  let text = doc.text;
  let rope = doc.rope;

  // Offline-mode enrichment: merge live catalog with text-scanned
  // sequences / types / extensions / functions / roles + AST-derived
  // tables so analysis rules still see something useful when the DB
  // isn't connected. Clone before the upcoming .await so the parking_lot
  // guard does not cross the suspend point (not Send).
  let live = state.catalog.read().clone();
  let derived = dsl_completion::source_tables::from_source(&cache.file, &text);
  let ws_offline = state.workspace_offline_snapshot();
  let cat = dsl_completion::source_tables::merge(
    &dsl_completion::source_tables::merge(&live, &derived),
    &ws_offline,
  );
  let raw = dsl_analysis::run(&text, &cache.file, &cache.scopes, &cat);

  // Cancellation check #1: skip mapping work if a newer didChange
  // already arrived. The next publish_for call will produce diagnostics
  // for the fresher buffer.
  if state.documents.is_stale(uri, snapshot_version) {
    tracing::debug!(uri = %uri, "diagnostics dropped: doc version superseded mid-analysis");
    return;
  }

  let diagnostics = raw
    .into_iter()
    .map(|d| Diagnostic {
      range: to_lsp_range(&rope, d.range),
      severity: Some(map_severity(d.severity)),
      code: Some(tower_lsp::lsp_types::NumberOrString::String(d.code.to_string())),
      source: Some("duck-sqllsp".into()),
      message: d.message,
      ..Default::default()
    })
    .collect::<Vec<_>>();

  // Cancellation check #2: right before we ship to the client.
  if state.documents.is_stale(uri, snapshot_version) {
    tracing::debug!(uri = %uri, "diagnostics dropped: doc version superseded before publish");
    return;
  }

  client.publish_diagnostics(uri.clone(), diagnostics, Some(snapshot_version)).await;
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
