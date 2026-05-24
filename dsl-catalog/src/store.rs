//! Thread-safe handle to the active catalog snapshot.
//!
//! The LSP server holds one [`CatalogStore`] and clones it into every
//! request handler. Cloning is cheap (one Arc bump). The inner RwLock
//! makes the trade-off explicit: many reads, occasional refresh.

use crate::model::Catalog;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct CatalogStore {
  inner: Arc<RwLock<Catalog>>,
}

impl CatalogStore {
  pub fn new() -> Self {
    Self::default()
  }

  /// Acquire a read guard. Cheap; multiple readers can run concurrently.
  pub fn read(&self) -> impl std::ops::Deref<Target = Catalog> + '_ {
    self.inner.read()
  }

  /// Replace the entire catalog snapshot. Used after a successful refresh.
  pub fn replace(&self, cat: Catalog) {
    *self.inner.write() = cat;
  }
}
