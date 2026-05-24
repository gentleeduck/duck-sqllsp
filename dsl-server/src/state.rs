//! Cross-request server state.
//!
//! Holds the catalog store, open documents, and the latest known LSP
//! config (connections + active). Cloned cheaply per request via Arcs
//! inside.

use crate::config::DuckSqllspConfig;
use crate::documents::DocumentStore;
use dsl_catalog::CatalogStore;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct ServerState {
    pub documents: DocumentStore,
    pub catalog: CatalogStore,
    pub config: Arc<RwLock<DuckSqllspConfig>>,
}

impl ServerState {
    pub fn new() -> Self { Self::default() }

    pub fn set_config(&self, cfg: DuckSqllspConfig) {
        *self.config.write() = cfg;
    }

    pub fn config_snapshot(&self) -> DuckSqllspConfig {
        self.config.read().clone()
    }
}
