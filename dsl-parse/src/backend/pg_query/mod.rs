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
        // pg_query reports cref.location etc relative to the chunk
        // we parse. Track the chunk's start byte in the source so we
        // can convert any chunk-local offsets into source offsets.
        let mut emitted = false;
        for raw in &result.protobuf.stmts {
          if let Some(stmt_node) = raw.stmt.as_ref().and_then(|s| s.node.as_ref()) {
            emitted = true;
            let mut kind = convert::statement(stmt_node, &chunk_owned);
            offset_ranges_in_kind(&mut kind, u32::from(range.start()));
            statements.push(Statement { range, kind });
          }
        }
        if !emitted {
          // Empty parse (whitespace / comments only).
          statements.push(Statement { range, kind: StatementKind::Unknown { text: chunk_owned } });
        }
      },
      Err(e) => {
        errors.push(ParseError { range, message: format!("pg_query: {e}") });
        statements.push(Statement { range, kind: StatementKind::Unknown { text: chunk } });
      },
    }
  }

  ParsedFile { statements, errors }
}

/// Walk every range field inside the parsed AST and add `offset` so the
/// chunk-relative byte positions emitted by pg_query become absolute
/// positions in the source buffer. Without this, diagnostics for a
/// column reference in the N-th statement display the line/column of
/// the very first chunk (off by `range.start()` bytes).
fn offset_ranges_in_kind(kind: &mut StatementKind, offset: u32) {
  use crate::ast::{Expr, Projection};
  fn fix_expr(e: &mut Expr, off: u32) {
    match e {
      Expr::Column { range, .. } if u32::from(range.len()) > 0 => {
        let s = u32::from(range.start()) + off;
        let en = u32::from(range.end()) + off;
        *range = text_size::TextRange::new(s.into(), en.into());
      },
      Expr::BinaryOp { left, right, .. } => {
        fix_expr(left, off);
        fix_expr(right, off);
      },
      Expr::Call { args, .. } => {
        for a in args {
          fix_expr(a, off);
        }
      },
      _ => {},
    }
  }
  fn fix_select(s: &mut crate::ast::SelectStmt, off: u32) {
    for p in &mut s.projections {
      if let Projection::Expr { expr, .. } = p {
        fix_expr(expr, off);
      }
    }
    if let Some(w) = &mut s.where_clause {
      fix_expr(w, off);
    }
    for j in &mut s.joins {
      if let Some(on) = &mut j.on {
        fix_expr(on, off);
      }
    }
  }
  match kind {
    StatementKind::Select(s) => fix_select(s, offset),
    StatementKind::Update(u) => {
      if let Some(w) = &mut u.where_clause {
        fix_expr(w, offset);
      }
      for (_t, e) in &mut u.assignments {
        fix_expr(e, offset);
      }
    },
    StatementKind::Delete(d) => {
      if let Some(w) = &mut d.where_clause {
        fix_expr(w, offset);
      }
    },
    _ => {},
  }
}
