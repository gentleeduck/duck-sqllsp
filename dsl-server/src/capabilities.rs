//! Server capability matrix.

use tower_lsp::lsp_types::{
  CodeActionKind, CodeActionOptions, CodeActionProviderCapability, CodeLensOptions, CompletionOptions,
  HoverProviderCapability, OneOf, RenameOptions, SemanticTokenType, SemanticTokensFullOptions, SemanticTokensLegend,
  SemanticTokensOptions, SemanticTokensServerCapabilities, ServerCapabilities, SignatureHelpOptions,
  TextDocumentSyncCapability, TextDocumentSyncKind, WorkDoneProgressOptions,
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

pub fn server_capabilities() -> ServerCapabilities {
  ServerCapabilities {
    text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
    completion_provider: Some(CompletionOptions {
      trigger_characters: Some(vec![".".into(), " ".into(), "(".into(), ",".into(), ":".into()]),
      resolve_provider: Some(false),
      ..Default::default()
    }),
    hover_provider: Some(HoverProviderCapability::Simple(true)),
    signature_help_provider: Some(SignatureHelpOptions {
      trigger_characters: Some(vec!["(".into(), ",".into()]),
      retrigger_characters: Some(vec![",".into()]),
      work_done_progress_options: WorkDoneProgressOptions::default(),
    }),
    definition_provider: Some(OneOf::Left(true)),
    document_symbol_provider: Some(OneOf::Left(true)),
    workspace_symbol_provider: Some(OneOf::Left(true)),
    selection_range_provider: Some(tower_lsp::lsp_types::SelectionRangeProviderCapability::Simple(true)),
    inlay_hint_provider: Some(tower_lsp::lsp_types::OneOf::Left(true)),
    code_lens_provider: Some(CodeLensOptions { resolve_provider: Some(false) }),
    document_formatting_provider: Some(OneOf::Left(true)),
    document_on_type_formatting_provider: Some(tower_lsp::lsp_types::DocumentOnTypeFormattingOptions {
      first_trigger_character: "\n".into(),
      more_trigger_character: None,
    }),
    references_provider: Some(OneOf::Left(true)),
    document_highlight_provider: Some(OneOf::Left(true)),
    folding_range_provider: Some(tower_lsp::lsp_types::FoldingRangeProviderCapability::Simple(true)),
    rename_provider: Some(OneOf::Right(RenameOptions {
      prepare_provider: Some(true),
      work_done_progress_options: WorkDoneProgressOptions::default(),
    })),
    semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
      legend: SemanticTokensLegend { token_types: SEMANTIC_LEGEND.to_vec(), token_modifiers: vec![] },
      range: Some(false),
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
