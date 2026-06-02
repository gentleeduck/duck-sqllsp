//! sql050: a column or table identifier in CREATE TABLE matches a PG
//! reserved keyword. Postgres still accepts it but forces every later
//! reference to be double-quoted -- a guaranteed paper-cut.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

// Pulled from PG 16 reserved-word list. Keep small; non-reserved words
// (NAME, USER, TYPE, ROLE, ...) are intentionally NOT in here because
// PG accepts them unquoted in identifier position.
const RESERVED: &[&str] = &[
  "ALL",
  "ANALYSE",
  "ANALYZE",
  "AND",
  "ANY",
  "ARRAY",
  "AS",
  "ASC",
  "ASYMMETRIC",
  "BOTH",
  "CASE",
  "CAST",
  "CHECK",
  "COLLATE",
  "COLUMN",
  "CONSTRAINT",
  "CREATE",
  "CURRENT_CATALOG",
  "CURRENT_DATE",
  "CURRENT_ROLE",
  "CURRENT_TIME",
  "CURRENT_TIMESTAMP",
  "CURRENT_USER",
  "DEFAULT",
  "DEFERRABLE",
  "DESC",
  "DISTINCT",
  "DO",
  "ELSE",
  "END",
  "EXCEPT",
  "FALSE",
  "FETCH",
  "FOR",
  "FOREIGN",
  "FROM",
  "GRANT",
  "GROUP",
  "HAVING",
  "IN",
  "INITIALLY",
  "INTERSECT",
  "INTO",
  "LATERAL",
  "LEADING",
  "LIMIT",
  "LOCALTIME",
  "LOCALTIMESTAMP",
  "NOT",
  "NULL",
  "OFFSET",
  "ON",
  "ONLY",
  "OR",
  "ORDER",
  "PLACING",
  "PRIMARY",
  "REFERENCES",
  "RETURNING",
  "SELECT",
  "SESSION_USER",
  "SOME",
  "SYMMETRIC",
  "TABLE",
  "THEN",
  "TO",
  "TRAILING",
  "TRUE",
  "UNION",
  "UNIQUE",
  "USER",
  "USING",
  "VARIADIC",
  "WHEN",
  "WHERE",
  "WINDOW",
  "WITH",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql050"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else {
      return;
    };
    let (stmt_start, stmt_end) = crate::stmt_bounds(stmt, source);
    let body = &source[stmt_start..stmt_end];
    if RESERVED.contains(&ct.table.name.to_ascii_uppercase().as_str()) {
      let range = if u32::from(ct.table.range.len()) > 0 {
        ct.table.range
      } else {
        find_after("CREATE TABLE", body, stmt_start, &ct.table.name)
          .or_else(|| find_after("CREATE TABLE IF NOT EXISTS", body, stmt_start, &ct.table.name))
          .unwrap_or(stmt.range)
      };
      out.push(Diagnostic {
        code: "sql050",
        severity: Severity::Hint,
        message: format!(
          "table name `{}` is a PG reserved word -- every reference will need double quotes",
          ct.table.name
        ),
        range,
      });
    }
    for col in &ct.columns {
      if RESERVED.contains(&col.name.to_ascii_uppercase().as_str()) {
        // Find the column name inside the table body. Walk
        // comma-separated entries.
        let range = find_column_name(body, stmt_start, &col.name).unwrap_or(stmt.range);
        out.push(Diagnostic {
          code: "sql050",
          severity: Severity::Hint,
          message: format!("column `{}` is a PG reserved word -- every reference will need double quotes", col.name),
          range,
        });
      }
    }
  }
}

fn find_after(prefix: &str, body: &str, body_offset: usize, name: &str) -> Option<text_size::TextRange> {
  let upper = body.to_ascii_uppercase();
  let idx = upper.find(&prefix.to_ascii_uppercase())?;
  let after = idx + prefix.len();
  let rest = &body[after..];
  let ws = rest.len() - rest.trim_start().len();
  let n_start = after + ws;
  let bytes = body.as_bytes();
  let mut e = n_start;
  while e < bytes.len()
    && (bytes[e].is_ascii_alphanumeric() || bytes[e] == b'_' || bytes[e] == b'.' || bytes[e] == b'"')
  {
    e += 1;
  }
  if body[n_start..e].eq_ignore_ascii_case(name) {
    Some(crate::range_at(body_offset + n_start, body_offset + e))
  } else {
    None
  }
}

fn find_column_name(body: &str, body_offset: usize, name: &str) -> Option<text_size::TextRange> {
  // Locate the column-list paren block.
  let open = body.find('(')?;
  let bytes = body.as_bytes();
  let mut depth = 0i32;
  let mut close = open;
  for (k, &b) in bytes.iter().enumerate().skip(open) {
    match b {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          close = k;
          break;
        }
      },
      _ => {},
    }
  }
  if close <= open {
    return None;
  }
  // Walk comma-separated entries, locate first identifier matching name.
  let inner_start = open + 1;
  let mut i = inner_start;
  while i < close {
    while i < close && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
      i += 1;
    }
    // Skip CONSTRAINT prefix entries.
    let s = i;
    while i < close && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
      i += 1;
    }
    let word = &body[s..i];
    if word.eq_ignore_ascii_case(name) {
      return Some(crate::range_at(body_offset + s, body_offset + i));
    }
    // Skip to next top-level `,`.
    let mut depth = 0i32;
    while i < close {
      match bytes[i] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b',' if depth == 0 => {
          i += 1;
          break;
        },
        _ => {},
      }
      i += 1;
    }
  }
  None
}
