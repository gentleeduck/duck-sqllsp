//! LSP wire layer for duck-sqllsp.
//!
//! Wraps every other crate behind a `tower-lsp::LanguageServer` impl.
//! `run_stdio` is the entry the `dsl-cli` binary calls; it sets up the
//! tokio runtime, registers the server, and pumps stdin/stdout.

pub mod backend;
pub mod capabilities;
pub mod config;
pub mod diagnostics;
pub mod documents;
pub mod handlers;
pub mod refresh;
pub mod state;

pub use backend::Backend;
pub use state::ServerState;
