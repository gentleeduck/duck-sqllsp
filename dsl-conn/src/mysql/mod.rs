//! MySQL / MariaDB driver.
//!
//! Uses `sqlx::mysql` for the wire protocol. Introspection runs against
//! `information_schema`, which MySQL implements with the same column
//! names as the ANSI spec so the queries port from PG with minor tweaks.

mod introspect;

use crate::driver::{Driver, DriverError};
use crate::spec::ConnectionSpec;
use async_trait::async_trait;
use dsl_catalog::Catalog;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub struct MysqlDriver {
    spec: ConnectionSpec,
    pool: Arc<OnceCell<MySqlPool>>,
}

impl MysqlDriver {
    pub fn new(spec: ConnectionSpec) -> Self {
        Self { spec, pool: Arc::new(OnceCell::new()) }
    }

    async fn pool(&self) -> Result<&MySqlPool, DriverError> {
        self.pool
            .get_or_try_init(|| async {
                MySqlPoolOptions::new()
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
impl Driver for MysqlDriver {
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
