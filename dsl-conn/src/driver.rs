//! The trait every backend implements.

use async_trait::async_trait;
use dsl_catalog::Catalog;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DriverError {
  #[error("connection error: {0}")]
  Connection(String),
  #[error("introspection error: {0}")]
  Introspect(String),
  #[error("unsupported driver: {0}")]
  Unsupported(String),
}

#[async_trait]
pub trait Driver: Send + Sync {
  /// Probe the connection; returns Ok if the DB is reachable.
  async fn ping(&self) -> Result<(), DriverError>;

  /// Fetch a fresh [`Catalog`] from the live database.
  async fn introspect(&self) -> Result<Catalog, DriverError>;
}
