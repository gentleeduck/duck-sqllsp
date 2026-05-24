//! `LanguageServer` impl bridging tower-lsp callbacks to our handlers.

use crate::capabilities::server_capabilities;
use crate::config;
use crate::handlers;
use crate::refresh;
use crate::state::ServerState;
use std::path::PathBuf;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

pub struct Backend {
  pub client: Client,
  pub state: ServerState,
}

impl Backend {
  pub fn new(client: Client) -> Self {
    Self { client, state: ServerState::new() }
  }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
  async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
    // Layer 1: initializationOptions from the editor.
    let mut effective = if let Some(opts) = params.initialization_options.clone() {
      config::parse(opts).duck_sqllsp
    } else {
      config::DuckSqllspConfig::default()
    };

    // Layer 2: project-level .duck-sqllsp.json walking upward from the
    // workspace root. Project file wins so a directory can ship its
    // own database without touching the editor.
    if let Some(root) = workspace_root(&params) {
      if let Some(proj) = config::load_project_config(&root) {
        effective.merge_from(proj);
      }
      self.state.set_workspace_root(root);
    }
    self.state.set_config(effective);
    // Workspace .sql scan: walks every *.sql in the workspace and
    // builds a baseline offline catalog so completion / hover /
    // diagnostics see tables/functions/types defined in files the
    // user has not yet opened. Live catalog (when connected) still
    // wins on collisions.
    let state = self.state.clone();
    tokio::spawn(async move {
      state.rescan_workspace_offline();
    });

    Ok(InitializeResult {
      server_info: Some(ServerInfo {
        name: "duck-sqllsp".to_string(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
      }),
      capabilities: server_capabilities(),
    })
  }

  async fn initialized(&self, _: InitializedParams) {
    self.client.log_message(MessageType::INFO, "duck-sqllsp initialized").await;
    let state = self.state.clone();
    let client = self.client.clone();
    tokio::spawn(async move {
      refresh::refresh_catalog(state, client).await;
    });
  }

  async fn shutdown(&self) -> Result<()> {
    Ok(())
  }

  async fn did_open(&self, params: DidOpenTextDocumentParams) {
    let td = params.text_document;
    // Project-config probe: walk up from the opened file path. Useful
    // when the editor did not set rootUri at initialize time.
    if let Ok(path) = td.uri.to_file_path() {
      if let Some(proj) = config::load_project_config(&path) {
        let mut cfg = self.state.config_snapshot();
        cfg.merge_from(proj);
        self.state.set_config(cfg);
        let state = self.state.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
          refresh::refresh_catalog(state, client).await;
        });
      }
    }
    let uri = td.uri.clone();
    self.state.documents.open(td.uri, td.text, td.version);
    crate::diagnostics::publish_for(&self.client, &self.state, &uri).await;
  }

  async fn did_change(&self, params: DidChangeTextDocumentParams) {
    if let Some(change) = params.content_changes.into_iter().next() {
      self.state.documents.update(&params.text_document.uri, change.text, params.text_document.version);
    }
    crate::diagnostics::publish_for(&self.client, &self.state, &params.text_document.uri).await;
  }

  async fn did_close(&self, params: DidCloseTextDocumentParams) {
    self.state.documents.close(&params.text_document.uri);
  }

  async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
    Ok(handlers::completion::run(&self.state, params))
  }

  async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
    Ok(handlers::hover::run(&self.state, params))
  }

  async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
    Ok(handlers::code_action::run(&self.state, params))
  }

  async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
    Ok(handlers::definition::run(&self.state, params))
  }

  async fn document_symbol(&self, params: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>> {
    Ok(handlers::document_symbol::run(&self.state, params))
  }

  async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
    Ok(handlers::formatting::run(&self.state, params))
  }

  async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
    Ok(handlers::references::run(&self.state, params))
  }

  async fn document_highlight(&self, params: DocumentHighlightParams) -> Result<Option<Vec<DocumentHighlight>>> {
    Ok(handlers::document_highlight::run(&self.state, params))
  }

  async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
    Ok(handlers::folding_range::run(&self.state, params))
  }

  async fn on_type_formatting(&self, params: DocumentOnTypeFormattingParams) -> Result<Option<Vec<TextEdit>>> {
    Ok(handlers::on_type_formatting::run(&self.state, params))
  }

  async fn goto_type_definition(
    &self,
    params: request::GotoTypeDefinitionParams,
  ) -> Result<Option<request::GotoTypeDefinitionResponse>> {
    Ok(handlers::type_definition::run(&self.state, params))
  }

  async fn linked_editing_range(
    &self,
    params: LinkedEditingRangeParams,
  ) -> Result<Option<LinkedEditingRanges>> {
    Ok(handlers::linked_editing::run(&self.state, params))
  }

  async fn prepare_call_hierarchy(
    &self,
    params: CallHierarchyPrepareParams,
  ) -> Result<Option<Vec<CallHierarchyItem>>> {
    Ok(handlers::call_hierarchy::prepare(&self.state, params))
  }

  async fn incoming_calls(
    &self,
    params: CallHierarchyIncomingCallsParams,
  ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
    Ok(handlers::call_hierarchy::incoming(&self.state, params))
  }

  async fn outgoing_calls(
    &self,
    params: CallHierarchyOutgoingCallsParams,
  ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
    Ok(handlers::call_hierarchy::outgoing(&self.state, params))
  }

  async fn execute_command(
    &self,
    params: ExecuteCommandParams,
  ) -> Result<Option<serde_json::Value>> {
    Ok(handlers::execute_command::run(&self.state, params).await)
  }

  async fn prepare_rename(&self, params: TextDocumentPositionParams) -> Result<Option<PrepareRenameResponse>> {
    Ok(handlers::rename::prepare(&self.state, params))
  }

  async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    Ok(handlers::rename::run(&self.state, params))
  }

  async fn semantic_tokens_full(&self, params: SemanticTokensParams) -> Result<Option<SemanticTokensResult>> {
    Ok(handlers::semantic_tokens::run(&self.state, params))
  }

  async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
    Ok(handlers::signature_help::run(&self.state, params))
  }

  async fn symbol(&self, params: WorkspaceSymbolParams) -> Result<Option<Vec<SymbolInformation>>> {
    Ok(handlers::workspace_symbol::run(&self.state, params))
  }

  async fn selection_range(&self, params: SelectionRangeParams) -> Result<Option<Vec<SelectionRange>>> {
    Ok(handlers::selection_range::run(&self.state, params))
  }

  async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
    Ok(handlers::inlay_hints::run(&self.state, params))
  }

  async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
    Ok(handlers::code_lens::run(&self.state, params))
  }

  async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
    let cfg = config::parse(params.settings);
    self.state.set_config(cfg.duck_sqllsp);
    let state = self.state.clone();
    let client = self.client.clone();
    tokio::spawn(async move {
      refresh::refresh_catalog(state, client).await;
    });
  }

  async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
    let mut config_changed = false;
    let mut sql_changed = false;
    for change in &params.changes {
      let Ok(path) = change.uri.to_file_path() else { continue };
      let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
      if name == ".duck-sqllsp.toml" || name == ".duck-sqllsp.json" {
        if let Some(proj) = config::load_project_config(&path) {
          let mut cfg = self.state.config_snapshot();
          cfg.merge_from(proj);
          self.state.set_config(cfg);
          config_changed = true;
        }
      }
      if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        if matches!(ext.to_ascii_lowercase().as_str(), "sql" | "pgsql" | "psql") {
          sql_changed = true;
        }
      }
    }
    if config_changed {
      let state = self.state.clone();
      let client = self.client.clone();
      tokio::spawn(async move {
        refresh::refresh_catalog(state, client).await;
      });
    }
    if sql_changed {
      // A .sql file on disk changed -- rescan the workspace so the
      // offline catalog stays in sync with what's actually on disk.
      let state = self.state.clone();
      tokio::spawn(async move {
        state.rescan_workspace_offline();
      });
    }
  }
}

/// Pick a workspace path from initialize params, preferring workspace_folders
/// then root_uri. Returns None when nothing usable is set.
fn workspace_root(params: &InitializeParams) -> Option<PathBuf> {
  if let Some(folders) = &params.workspace_folders {
    for f in folders {
      if let Ok(p) = f.uri.to_file_path() {
        return Some(p);
      }
    }
  }
  #[allow(deprecated)]
  if let Some(uri) = &params.root_uri {
    if let Ok(p) = uri.to_file_path() {
      return Some(p);
    }
  }
  None
}
