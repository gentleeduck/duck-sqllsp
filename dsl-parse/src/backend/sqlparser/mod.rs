//! sqlparser backend.
//!
//! Slices the source on top-level semicolons (via [`crate::split`]) and
//! parses each chunk independently so one bad statement doesn't poison
//! the rest of the file. The resulting upstream AST is then mapped to our
//! internal [`Statement`](crate::ast::Statement) by [`convert`].

mod convert;
mod dialect_pick;

use crate::ast::{Statement, StatementKind};
use crate::dialect::Dialect;
use crate::error::ParseError;
use crate::parsed_file::ParsedFile;
use crate::split::split_statements;
use sqlparser::parser::Parser;

/// Entry point. Identical signature to the crate-level `parse`; the
/// feature flag in `lib.rs` decides whether to call this.
pub fn parse(source: &str, dialect: Dialect) -> ParsedFile {
  let d = dialect_pick::pick(dialect);
  let mut statements = Vec::new();
  let mut errors = Vec::new();

  for (chunk, range) in split_statements(source) {
    match Parser::parse_sql(&*d, &chunk) {
      Ok(parsed) => {
        for stmt in parsed {
          statements.push(Statement { range, kind: convert::statement(stmt, &chunk) });
        }
      },
      Err(e) => {
        errors.push(ParseError { range, message: e.to_string() });
        statements.push(Statement { range, kind: StatementKind::Unknown { text: chunk } });
      },
    }
  }

  ParsedFile { statements, errors }
}
