//! SQLite driver.
//!
//! Uses `sqlx::sqlite` for the file connection. Introspection runs against
//! `sqlite_master` plus a few `PRAGMA` calls (`table_info`, `index_list`,
//! `index_info`, `foreign_key_list`) because SQLite doesn't expose a
//! complete `information_schema`.

mod introspect;

use crate::driver::{Driver, DriverError};
use crate::spec::ConnectionSpec;
use async_trait::async_trait;
use dsl_catalog::Catalog;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub struct SqliteDriver {
    spec: ConnectionSpec,
    pool: Arc<OnceCell<SqlitePool>>,
}

impl SqliteDriver {
    pub fn new(spec: ConnectionSpec) -> Self {
        Self { spec, pool: Arc::new(OnceCell::new()) }
    }

    async fn pool(&self) -> Result<&SqlitePool, DriverError> {
        self.pool
            .get_or_try_init(|| async {
                SqlitePoolOptions::new()
                    .max_connections(2)
                    .acquire_timeout(std::time::Duration::from_secs(5))
                    .connect(&self.spec.url())
                    .await
                    .map_err(|e| DriverError::Connection(e.to_string()))
            })
            .await
    }
}

#[async_trait]
impl Driver for SqliteDriver {
    async fn ping(&self) -> Result<(), DriverError> {
        let pool = self.pool().await?;
        sqlx::query("SELECT 1")
            .execute(pool)
            .await
            .map_err(|e| DriverError::Connection(e.to_string()))?;
        Ok(())
    }

    async fn introspect(&self) -> Result<Catalog, DriverError> {
        let pool = self.pool().await?;
        introspect::run(pool, &self.spec).await
    }
}
