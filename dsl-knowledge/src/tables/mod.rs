//! Knowledge-base tables, lazily initialised on first use.
//!
//! Each sub-module owns one table:
//!   - [`keywords`] -- SQL keywords (SELECT, FROM, ...).
//!   - [`types`]    -- data types (UUID, TIMESTAMPTZ, ...).
//!   - [`functions`] -- built-in functions (count, now, gen_random_uuid, ...).
//!
//! Each table is keyed by canonical case: keywords + types use uppercase,
//! functions use lowercase. The [`lookup`] helper tries all three.

pub mod functions;
pub mod keywords;
pub mod types;

use crate::entry::Entry;
use once_cell::sync::Lazy;
use std::collections::HashMap;

pub fn keywords() -> &'static HashMap<&'static str, Entry> {
    static MAP: Lazy<HashMap<&'static str, Entry>> = Lazy::new(keywords::build);
    &MAP
}

pub fn types() -> &'static HashMap<&'static str, Entry> {
    static MAP: Lazy<HashMap<&'static str, Entry>> = Lazy::new(types::build);
    &MAP
}

pub fn functions() -> &'static HashMap<&'static str, Entry> {
    static MAP: Lazy<HashMap<&'static str, Entry>> = Lazy::new(functions::build);
    &MAP
}

/// Try keyword (uppercase), then type (uppercase), then function (lowercase).
/// Returns the first match.
pub fn lookup(token: &str) -> Option<&'static Entry> {
    let upper = token.to_ascii_uppercase();
    if let Some(e) = keywords().get(upper.as_str()) {
        return Some(e);
    }
    if let Some(e) = types().get(upper.as_str()) {
        return Some(e);
    }
    let lower = token.to_ascii_lowercase();
    functions().get(lower.as_str())
}
