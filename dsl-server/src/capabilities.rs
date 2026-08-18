//! Server capability matrix.

use tower_lsp::lsp_types::{
  CodeActionKind, CodeActionOptions, CodeActionProviderCapability, CodeLensOptions, CompletionOptions,
  DiagnosticOptions, DiagnosticServerCapabilities, HoverProviderCapability, OneOf, RenameOptions,
  SemanticTokenModifier, SemanticTokenType, SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
  SemanticTokensServerCapabilities, ServerCapabilities, SignatureHelpOptions, TextDocumentSyncCapability,
  TextDocumentSyncKind, WorkDoneProgressOptions,
};

/// Order MUST match the `Tok` enum in `handlers/semantic_tokens.rs`.
pub const SEMANTIC_LEGEND: &[SemanticTokenType] = &[
  SemanticTokenType::KEYWORD,
  SemanticTokenType::TYPE,
  SemanticTokenType::FUNCTION,
  SemanticTokenType::CLASS,    // tables
  SemanticTokenType::PROPERTY, // columns
  SemanticTokenType::VARIABLE, // NEW/OLD/locals
  SemanticTokenType::PARAMETER,
  SemanticTokenType::STRING,
  SemanticTokenType::NUMBER,
  SemanticTokenType::COMMENT,
  SemanticTokenType::OPERATOR,
];

/// Bit order MUST match the `modbit` constants in
/// `handlers/semantic_tokens.rs`.
pub const SEMANTIC_MODIFIERS: &[SemanticTokenModifier] =
  &[SemanticTokenModifier::DECLARATION, SemanticTokenModifier::DEFINITION, SemanticTokenModifier::DEFAULT_LIBRARY];

pub fn server_capabilities() -> ServerCapabilities {
  ServerCapabilities {
    // Incremental sync: the editor ships only the edited range instead
    // of the whole buffer on every keystroke, and we splice it into the
    // rope in place. Matters most on the multi-thousand-line migration
    // files this server is aimed at.
    text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::INCREMENTAL)),
    completion_provider: Some(CompletionOptions {
      trigger_characters: Some(vec![".".into(), " ".into(), "(".into(), ",".into(), ":".into()]),
      // Documentation for built-ins and snippets is deferred to
      // `completionItem/resolve` -- see `handlers/completion_resolve.rs`.
      resolve_provider: Some(true),
      ..Default::default()
    }),
    hover_provider: Some(HoverProviderCapability::Simple(true)),
    // LSP 3.17 pull diagnostics. `inter_file_dependencies` is true
    // because a document's findings depend on the merged catalog, which
    // other buffers and the workspace `.sql` scan feed -- editing one
    // file can change another's sql001/sql002 results.
    // `workspace_diagnostics` stays false: we have no cheap way to
    // enumerate diagnostics for unopened files, and claiming it would
    // make clients ask for exactly that.
    diagnostic_provider: Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
      identifier: Some("duck-sqllsp".into()),
      inter_file_dependencies: true,
      workspace_diagnostics: false,
      work_done_progress_options: WorkDoneProgressOptions::default(),
    })),
    signature_help_provider: Some(SignatureHelpOptions {
      trigger_characters: Some(vec!["(".into(), ",".into()]),
      retrigger_characters: Some(vec![",".into()]),
      work_done_progress_options: WorkDoneProgressOptions::default(),
    }),
    definition_provider: Some(OneOf::Left(true)),
    type_definition_provider: Some(tower_lsp::lsp_types::TypeDefinitionProviderCapability::Simple(true)),
    document_symbol_provider: Some(OneOf::Left(true)),
    workspace_symbol_provider: Some(OneOf::Left(true)),
    selection_range_provider: Some(tower_lsp::lsp_types::SelectionRangeProviderCapability::Simple(true)),
    inlay_hint_provider: Some(tower_lsp::lsp_types::OneOf::Left(true)),
    code_lens_provider: Some(CodeLensOptions { resolve_provider: Some(false) }),
    document_formatting_provider: Some(OneOf::Left(true)),
    // Format-selection. Snaps outward to whole statements -- see
    // `handlers/range_formatting.rs` for why sub-statement ranges
    // can't be formatted in isolation.
    document_range_formatting_provider: Some(OneOf::Left(true)),
    document_on_type_formatting_provider: Some(tower_lsp::lsp_types::DocumentOnTypeFormattingOptions {
      first_trigger_character: "\n".into(),
      more_trigger_character: None,
    }),
    // psql `\i` includes, `COPY ... FROM/TO '<file>'`, and URLs in
    // comments. File targets are resolved eagerly (and dropped when the
    // path does not exist), so there is nothing left to resolve.
    document_link_provider: Some(tower_lsp::lsp_types::DocumentLinkOptions {
      resolve_provider: Some(false),
      work_done_progress_options: WorkDoneProgressOptions::default(),
    }),
    references_provider: Some(OneOf::Left(true)),
    document_highlight_provider: Some(OneOf::Left(true)),
    folding_range_provider: Some(tower_lsp::lsp_types::FoldingRangeProviderCapability::Simple(true)),
    linked_editing_range_provider: Some(tower_lsp::lsp_types::LinkedEditingRangeServerCapabilities::Simple(true)),
    call_hierarchy_provider: Some(tower_lsp::lsp_types::CallHierarchyServerCapability::Simple(true)),
    execute_command_provider: Some(tower_lsp::lsp_types::ExecuteCommandOptions {
      commands: crate::handlers::execute_command::SUPPORTED.iter().map(|s| s.to_string()).collect(),
      work_done_progress_options: tower_lsp::lsp_types::WorkDoneProgressOptions::default(),
    }),
    rename_provider: Some(OneOf::Right(RenameOptions {
      prepare_provider: Some(true),
      work_done_progress_options: WorkDoneProgressOptions::default(),
    })),
    semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
      legend: SemanticTokensLegend {
        token_types: SEMANTIC_LEGEND.to_vec(),
        token_modifiers: SEMANTIC_MODIFIERS.to_vec(),
      },
      // Range requests let a client colour just the viewport of a large
      // migration file instead of waiting on a whole-buffer pass.
      range: Some(true),
      full: Some(SemanticTokensFullOptions::Bool(true)),
      work_done_progress_options: WorkDoneProgressOptions::default(),
    })),
    code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
      code_action_kinds: Some(vec![CodeActionKind::QUICKFIX, CodeActionKind::REFACTOR]),
      work_done_progress_options: WorkDoneProgressOptions::default(),
      resolve_provider: Some(false),
    })),
    ..Default::default()
  }
}
