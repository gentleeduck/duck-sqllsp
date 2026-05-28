//! Cross-request server state.
//!
//! Holds the catalog store, open documents, and the latest known LSP
//! config (connections + active). Cloned cheaply per request via Arcs
//! inside.

use crate::config::DuckSqllspConfig;
use crate::documents::DocumentStore;
use dsl_catalog::{Catalog, CatalogStore};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct ServerState {
  pub documents: DocumentStore,
  pub catalog: CatalogStore,
  pub config: Arc<RwLock<DuckSqllspConfig>>,
  /// Project root (from initialize). Used by the workspace .sql scan
  /// to derive an offline catalog from every file on disk -- not just
  /// the open buffers.
  pub workspace_root: Arc<RwLock<Option<PathBuf>>>,
  /// Cached offline catalog built from a recursive .sql scan of the
  /// workspace. Refreshed at initialize + on did_change_watched_files
  /// for .sql files. Indexed by file path so partial updates skip
  /// re-parsing unchanged files.
  pub workspace_offline: Arc<RwLock<Catalog>>,
  /// Lowercased `clientInfo.name` from the `initialize` request. Used
  /// to gate features that require client-side wiring -- the Run /
  /// EXPLAIN CodeLens commands need a corresponding handler in the
  /// editor, which only the VS Code extension currently ships.
  pub client_name: Arc<RwLock<Option<String>>>,
}

impl ServerState {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn set_config(&self, cfg: DuckSqllspConfig) {
    *self.config.write() = cfg;
  }

  pub fn config_snapshot(&self) -> DuckSqllspConfig {
    self.config.read().clone()
  }

  pub fn set_workspace_root(&self, root: PathBuf) {
    *self.workspace_root.write() = Some(root);
  }

  pub fn set_client_name(&self, name: Option<String>) {
    *self.client_name.write() = name.map(|s| s.to_lowercase());
  }

  /// True when the connected editor is the VS Code extension (or one of
  /// its forks). These are the clients that ship a handler for the
  /// `duck-sqllsp.runQuery` / `.explainQuery` / `.addLimit` CodeLens
  /// commands. Other clients (nvim, helix, ...) would just see a
  /// "command not found" toast on click, so we suppress the runnable
  /// lenses for them.
  pub fn client_supports_runnable_codelens(&self) -> bool {
    match self.client_name.read().as_deref() {
      Some(name) => {
        name.contains("visual studio code")
          || name.contains("code - oss")
          || name.contains("vscode")
          || name.contains("cursor")
          || name.contains("windsurf")
      },
      None => false,
    }
  }

  /// Walk the workspace root for .sql files (capped for safety) and
  /// rebuild the cached offline catalog. Cheap: each file is parsed +
  /// fed to from_source so tables / sequences / functions / types /
  /// extensions / roles all show up workspace-wide even when the user
  /// hasn't opened the defining file yet.
  pub fn rescan_workspace_offline(&self) {
    let Some(root) = self.workspace_root.read().clone() else { return };
    let mut cat: Catalog = dsl_catalog::Catalog {
      version: dsl_catalog::CATALOG_VERSION,
      connection_id: "<workspace-scan>".into(),
      ..Default::default()
    };
    // 4 MiB per file matches Document::MAX_DOC_BYTES; 5_000 file cap
    // keeps the scan bounded on very large monorepos.
    const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
    const MAX_FILES: usize = 5000;
    let mut count = 0usize;
    walk_sql_files(&root, MAX_FILES, &mut count, &mut |path| {
      let Ok(meta) = std::fs::metadata(path) else { return };
      if meta.len() > MAX_FILE_BYTES {
        return;
      }
      let Ok(text) = std::fs::read_to_string(path) else { return };
      let file = dsl_parse::parse(&text, dsl_parse::Dialect::Postgres);
      let derived = dsl_completion::source_tables::from_source(&file, &text);
      cat = dsl_completion::source_tables::merge(&cat, &derived);
    });
    *self.workspace_offline.write() = cat;
  }

  /// Snapshot the workspace offline catalog (rescan was done at
  /// initialize / on watched-file change).
  pub fn workspace_offline_snapshot(&self) -> Catalog {
    self.workspace_offline.read().clone()
  }
}

/// Walk `root` recursively, calling `f` for each *.sql file. Skips
/// hidden dirs (`.git`, `.svn`, `node_modules`, `target`, etc) to
/// keep the scan from drowning in noise.
fn walk_sql_files(root: &Path, cap: usize, count: &mut usize, f: &mut impl FnMut(&Path)) {
  if *count >= cap {
    return;
  }
  let Ok(rd) = std::fs::read_dir(root) else { return };
  for entry in rd.flatten() {
    if *count >= cap {
      return;
    }
    let path = entry.path();
    if let Some(name) = path.file_name().and_then(|s| s.to_str())
      && (name.starts_with('.') || matches!(name, "node_modules" | "target" | "dist" | "build" | "vendor" | "out"))
    {
      continue;
    }
    if path.is_dir() {
      walk_sql_files(&path, cap, count, f);
    } else if let Some(ext) = path.extension().and_then(|s| s.to_str())
      && matches!(ext.to_ascii_lowercase().as_str(), "sql" | "pgsql" | "psql")
    {
      *count += 1;
      f(&path);
    }
  }
}
