//! SQL parser facade for duck-sqllsp.
//!
//! Wraps existing battle-tested SQL parsers (currently [`sqlparser`], with
//! `pg_query` planned behind a feature flag) and normalises their output
//! into a single small AST. Every downstream crate in this workspace
//! depends only on this internal AST, so changing parser backends is a
//! one-crate diff.
//!
//! Public surface:
//!   - [`parse`] -- top-level entry point.
//!   - [`Dialect`] -- which SQL flavour to use.
//!   - The AST types in [`ast`] -- `Statement`, `SelectStmt`, etc.
//!   - [`ParsedFile`], [`ParseError`] -- result envelopes.

pub mod ast;
pub mod dialect;
pub mod error;
pub mod parsed_file;
pub mod split;

pub mod backend;

pub use ast::*;
pub use dialect::Dialect;
pub use error::ParseError;
pub use parsed_file::ParsedFile;

/// Parse a whole SQL document. Always succeeds; per-statement errors are
/// collected into [`ParsedFile::errors`]. The real Postgres parser
/// (`pg_query`) is the default; `sqlparser` is the pure-Rust fallback.
pub fn parse(source: &str, dialect: Dialect) -> ParsedFile {
  #[cfg(feature = "pg_query_backend")]
  {
    return backend::pg_query::parse(source, dialect);
  }
  #[cfg(all(not(feature = "pg_query_backend"), feature = "sqlparser"))]
  {
    return backend::sqlparser::parse(source, dialect);
  }
  #[cfg(all(not(feature = "pg_query_backend"), not(feature = "sqlparser")))]
  {
    let range = text_size::TextRange::up_to(text_size::TextSize::of(source));
    ParsedFile {
      statements: vec![Statement { range, kind: StatementKind::Unknown { text: source.to_string() } }],
      errors: vec![ParseError { range, message: "no parser backend enabled".into() }],
    }
  }
}
