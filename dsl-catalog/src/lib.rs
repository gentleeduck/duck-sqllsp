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

/// Strip schema qualifier from a type string for display.
///
/// PG's `information_schema.columns.data_type` and other introspection
/// queries can return types qualified as `pg_catalog.varchar`,
/// `pg_catalog.text`, etc. We always want to show the bare type name
/// in hover / completion / inlay hints. Also strips `public.` for
/// user-defined types where the schema is implicit.
pub fn display_type(s: &str) -> &str {
  let s = s.trim();
  for prefix in ["pg_catalog.", "PG_CATALOG.", "public.", "PUBLIC."] {
    if let Some(rest) = s.strip_prefix(prefix) {
      return rest;
    }
  }
  s
}
