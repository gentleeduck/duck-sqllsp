//! `textDocument/diagnostic` handler -- LSP 3.17 pull diagnostics.
//!
//! The push model (`publishDiagnostics`) makes the *server* guess when
//! the client wants diagnostics. That guess is wrong in both directions:
//! we re-analyse on every keystroke for a file the user may not even be
//! looking at, and we have no way to answer "what are the diagnostics
//! for this file right now?" for a file that was never opened.
//!
//! Pull inverts it. The client asks per document, on its own cadence
//! (VS Code: on open, on idle after typing, on focus change), and we
//! answer with either a full report or -- the interesting case -- an
//! `Unchanged` report keyed by the result id we handed out last time.
//!
//! The result id folds together the document bytes and
//! [`ServerState::analysis_generation`], the counter bumped whenever a
//! *non-text* analysis input moves (config reload, live catalog swap,
//! workspace `.sql` rescan). So an unchanged buffer skips the analysis
//! run entirely, but a schema refresh that clears a sql001
//! unresolved-table finding still invalidates the client's cache.
//!
//! `related_documents` is left empty: our cross-file knowledge flows
//! through the merged catalog rather than through per-file diagnostic
//! dependencies, so there is no second document whose diagnostics we
//! could produce as a side effect of analysing this one.

use crate::state::ServerState;
use tower_lsp::lsp_types::{
  DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult, FullDocumentDiagnosticReport,
  RelatedFullDocumentDiagnosticReport, RelatedUnchangedDocumentDiagnosticReport, UnchangedDocumentDiagnosticReport,
};

pub fn run(state: &ServerState, params: DocumentDiagnosticParams) -> DocumentDiagnosticReportResult {
  let uri = &params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("diagnostic", uri);

  let current_id = crate::diagnostics::result_id(state, uri);

  // Cache hit: same bytes, same catalog/config generation. Answer
  // without touching the analysis engine.
  if let (Some(prev), Some(current)) = (params.previous_result_id.as_deref(), current_id.as_deref())
    && prev == current
  {
    return DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Unchanged(
      RelatedUnchangedDocumentDiagnosticReport {
        related_documents: None,
        unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport { result_id: current.to_string() },
      },
    ));
  }

  // `compute` returns None for an unknown URI or a mid-flight
  // supersede. Either way an empty full report is the honest answer --
  // withholding one leaves the client showing whatever it had before,
  // which for a closed document is a stale overlay.
  let items = crate::diagnostics::compute(state, uri).map(|(d, _)| d).unwrap_or_default();

  DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
    related_documents: None,
    full_document_diagnostic_report: FullDocumentDiagnosticReport { result_id: current_id, items },
  }))
}
