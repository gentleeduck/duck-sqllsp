//! JSON persistence under the user's XDG cache directory.
//!
//! One file per connection so multiple workspaces sharing a single nvim
//! install don't fight over a shared file. Version-stamped via
//! [`CATALOG_VERSION`] so older formats can be detected and re-fetched.

use crate::model::Catalog;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Bump when the on-disk schema changes.
pub const CATALOG_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum PersistError {
    #[error("no usable cache dir found")]
    NoCacheDir,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Resolve the canonical cache path for a given connection id.
pub fn cache_path_for(connection_id: &str) -> Result<PathBuf, PersistError> {
    let proj = directories::ProjectDirs::from("org", "gentleduck", "duck-sqllsp")
        .ok_or(PersistError::NoCacheDir)?;
    Ok(proj.cache_dir().join("catalogs").join(format!("{connection_id}.json")))
}

pub fn save(cat: &Catalog) -> Result<(), PersistError> {
    let path = cache_path_for(&cat.connection_id)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(cat)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load(connection_id: &str) -> Result<Catalog, PersistError> {
    let path = cache_path_for(connection_id)?;
    load_from(&path)
}

pub fn load_from(path: &Path) -> Result<Catalog, PersistError> {
    let bytes = std::fs::read(path)?;
    let cat: Catalog = serde_json::from_slice(&bytes)?;
    Ok(cat)
}
