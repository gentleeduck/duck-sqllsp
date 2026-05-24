//! Context-aware SQL completion engine.
//!
//! Public entry point is [`complete`]: given the source text, parsed
//! statements, resolved scopes, catalog snapshot, and cursor offset, it
//! returns a flat list of [`Item`]s. The LSP server crate maps each
//! [`Item`] onto an LSP `CompletionItem`; using a plain struct here keeps
//! this crate free of LSP wire dependencies and unit-testable in
//! milliseconds.

pub mod context;
pub mod create_index;
pub mod create_table;
pub mod engine;
pub mod fallback;
pub mod item;
pub mod phase;
pub mod plpgsql_locals;
pub mod render;
pub mod source_tables;
pub mod sources;

pub use engine::complete;
pub use item::{Item, ItemKind};
