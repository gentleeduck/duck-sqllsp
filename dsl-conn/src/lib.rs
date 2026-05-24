//! DB connection layer.
//!
//! Defines a small [`Driver`] trait every backend implements. Provided
//! impls live behind Cargo features so a slim build can include only the
//! driver(s) it needs.
//!
//! Build a driver from a [`ConnectionSpec`] with [`build`].

pub mod driver;
pub mod spec;

#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use driver::{Driver, DriverError};
pub use spec::ConnectionSpec;

/// Build the right driver for a [`ConnectionSpec`]. Returns
/// `Err(Unsupported)` if the matching Cargo feature isn't enabled.
pub fn build(spec: &ConnectionSpec) -> Result<Box<dyn Driver>, DriverError> {
  match spec.driver() {
    #[cfg(feature = "postgres")]
    "postgres" => Ok(Box::new(postgres::PostgresDriver::new(spec.clone()))),
    #[cfg(feature = "mysql")]
    "mysql" => Ok(Box::new(mysql::MysqlDriver::new(spec.clone()))),
    #[cfg(feature = "sqlite")]
    "sqlite" => Ok(Box::new(sqlite::SqliteDriver::new(spec.clone()))),
    other => Err(DriverError::Unsupported(other.to_string())),
  }
}
