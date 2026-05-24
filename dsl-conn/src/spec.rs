//! ConnectionSpec: the shape the LSP receives from
//! `initializationOptions` and `.duck-sqllsp.toml`.
//!
//! Single source of truth: a URL string. The driver is inferred from
//! the URL scheme (`postgres://...`, `postgresql://...`, `mysql://...`,
//! `mariadb://...`, `sqlite://...`). No separate host / port / user /
//! password / database fields -- the URL carries them.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionSpec {
  pub name: String,
  pub url: String,
}

impl ConnectionSpec {
  /// URL passed straight to sqlx.
  pub fn url(&self) -> String {
    self.url.clone()
  }

  /// Extract the driver from the URL scheme. Returns the canonical
  /// driver name [`crate::build`] matches on.
  pub fn driver(&self) -> &'static str {
    let lower = self.url.to_ascii_lowercase();
    if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
      "postgres"
    } else if lower.starts_with("mysql://") || lower.starts_with("mariadb://") {
      "mysql"
    } else if lower.starts_with("sqlite://") || lower.starts_with("sqlite:") {
      "sqlite"
    } else {
      "unknown"
    }
  }
}
