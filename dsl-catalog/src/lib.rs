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

pub mod lookup;
pub mod model;
pub mod persist;
pub mod store;

pub use model::*;
pub use persist::{CATALOG_VERSION, PersistError, cache_path_for, load, load_from, save};
pub use store::CatalogStore;
