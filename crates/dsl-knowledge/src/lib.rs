//! Curated SQL knowledge base used by duck-sqllsp completion and hover.
//!
//! Public surface:
//!   - [`Entry`], [`Kind`] -- shape of one knowledge entry.
//!   - [`keywords`], [`types`], [`functions`] -- lazily-initialized lookup tables.
//!   - [`lookup`] -- try keyword / type / function in order.
//!   - [`render_markdown`] -- produce the markdown blob shown in hover and cmp docs.
//!
//! All data lives under `tables/`; the `lib.rs` surface is intentionally tiny.

pub mod entry;
pub mod tables;
pub mod render;

pub use entry::{Entry, Kind, PG_DOCS_BASE};
pub use render::{render_markdown, wrap_paragraphs};
pub use tables::{functions, keywords, lookup, types};
