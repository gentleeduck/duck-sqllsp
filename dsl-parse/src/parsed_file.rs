//! Top-level result type returned by [`parse`](crate::parse).

use crate::ast::Statement;
use crate::error::ParseError;
use serde::Serialize;

/// A whole SQL file, sliced into top-level statements.
///
/// `statements.len() == split_statements(source).len()`. Failed statements
/// are present as `StatementKind::Unknown` so callers can render the
/// surrounding statements normally; the matching `ParseError` lives in
/// [`errors`](ParsedFile::errors).
#[derive(Debug, Clone, Serialize)]
pub struct ParsedFile {
    pub statements: Vec<Statement>,
    pub errors: Vec<ParseError>,
}
