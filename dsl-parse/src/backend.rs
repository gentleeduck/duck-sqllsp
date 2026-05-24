//! Parser backends. `pg_query` (libpg_query FFI) is the default --
//! 100% Postgres syntax coverage. `sqlparser` (pure Rust) is the
//! fallback for environments without a C toolchain.

#[cfg(feature = "sqlparser")]
pub mod sqlparser;

#[cfg(feature = "pg_query_backend")]
pub mod pg_query;
