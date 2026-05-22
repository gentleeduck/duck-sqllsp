//! pg_query backend.
//!
//! Uses libpg_query (the real Postgres parser, exposed via FFI) so we
//! get 100% PG syntax coverage including features that sqlparser-rs
//! doesn't yet support (`ON DELETE SET NULL (col)`, MERGE, FETCH FIRST,
//! lateral derived tables, ...). The conversion is intentionally
//! lossy -- we only extract the slots the rest of the workspace cares
//! about (CREATE TABLE columns, FROM tables, projections, WHERE, etc).
//! Anything richer than that lands in `StatementKind::Unknown { text }`
//! so downstream features still see the raw SQL.

mod convert;

use crate::ast::{Statement, StatementKind};
use crate::dialect::Dialect;
use crate::error::ParseError;
use crate::parsed_file::ParsedFile;
use crate::split::split_statements;

pub fn parse(source: &str, _dialect: Dialect) -> ParsedFile {
    let mut statements = Vec::new();
    let mut errors = Vec::new();

    for (chunk, range) in split_statements(source) {
        match pg_query::parse(&chunk) {
            Ok(result) => {
                let chunk_owned = chunk.clone();
                let mut emitted = false;
                for raw in &result.protobuf.stmts {
                    if let Some(stmt_node) = raw.stmt.as_ref().and_then(|s| s.node.as_ref()) {
                        emitted = true;
                        statements.push(Statement {
                            range,
                            kind: convert::statement(stmt_node, &chunk_owned),
                        });
                    }
                }
                if !emitted {
                    // Empty parse (whitespace / comments only).
                    statements.push(Statement {
                        range,
                        kind: StatementKind::Unknown { text: chunk_owned },
                    });
                }
            }
            Err(e) => {
                errors.push(ParseError {
                    range,
                    message: format!("pg_query: {e}"),
                });
                statements.push(Statement {
                    range,
                    kind: StatementKind::Unknown { text: chunk },
                });
            }
        }
    }

    ParsedFile { statements, errors }
}
