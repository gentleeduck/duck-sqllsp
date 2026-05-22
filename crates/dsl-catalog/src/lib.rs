//! Schema cache for duck-sqllsp.
//!
//! The catalog mirrors a database's structural metadata (schemas, tables,
//! views, columns, constraints, indexes, functions) in memory and on
//! disk. `dsl-conn` populates it via async introspection queries; every
//! downstream provider (`dsl-completion`, `dsl-hover`, `dsl-analysis`)
//! reads from here so the database is touched only on explicit refresh.
//!
//! On-disk format is a versioned JSON file per connection, kept under the
//! user's XDG cache directory. See [`persist::cache_path_for`].

pub mod model;
pub mod store;
pub mod persist;
pub mod lookup;

pub use model::*;
pub use persist::{cache_path_for, load, load_from, save, PersistError, CATALOG_VERSION};
pub use store::CatalogStore;
