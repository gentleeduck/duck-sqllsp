//! Postgres driver.
//!
//! Uses `sqlx::postgres` for the wire protocol. Introspection is done via
//! `information_schema` plus a handful of `pg_catalog` joins for richer
//! detail than the standard catalog views provide.

mod introspect;

use crate::driver::{Driver, DriverError};
use crate::spec::ConnectionSpec;
use async_trait::async_trait;
use dsl_catalog::Catalog;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub struct PostgresDriver {
    spec: ConnectionSpec,
    pool: Arc<OnceCell<PgPool>>,
}

impl PostgresDriver {
    pub fn new(spec: ConnectionSpec) -> Self {
        Self { spec, pool: Arc::new(OnceCell::new()) }
    }

    async fn pool(&self) -> Result<&PgPool, DriverError> {
        self.pool
            .get_or_try_init(|| async {
                PgPoolOptions::new()
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
impl Driver for PostgresDriver {
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
