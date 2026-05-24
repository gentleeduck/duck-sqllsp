//! sql039: `INSERT INTO t (col1, col2) VALUES (lit1, lit2)` literal
//! types must match the target column types.
//!
//! Conservative: only flags literal kinds we can classify with high
//! confidence (string / integer / float / boolean / NULL). Anything
//! else (function call, expression, cast) is skipped.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LitKind {
  Str,
  Int,
  Float,
  Bool,
  Null,
  Unknown,
}

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql039"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(i) = &stmt.kind else {
      return;
    };
    if i.columns.is_empty() {
      return;
    }
    let Some(t) = catalog.find_table(i.table.schema.as_deref(), &i.table.name) else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(values_at) = upper.find("VALUES") else { return };
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut k = values_at + 6;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= n || bytes[k] != b'(' {
      return;
    }
    let Some(close) = match_paren(bytes, k) else { return };
    let tuple = &body[k + 1..close];
    let values = split_top_level_commas(tuple);
    if values.len() != i.columns.len() {
      return;
    } // sql038 territory

    for (col_name, raw_val) in i.columns.iter().zip(values.iter()) {
      let lit = classify_literal(raw_val.trim());
      if matches!(lit, LitKind::Unknown | LitKind::Null) {
        continue;
      }
      // Find column type from catalog.
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
      if !compatible(lit, &col.data_type) {
        // Narrow the range to the offending literal: compute its
        // byte offset relative to `body` via pointer arithmetic on
        // the slice we got back from split_top_level_commas. Then
        // shift by `start` (the stmt's start in the source) to get
        // an absolute byte range. Falls back to stmt.range if the
        // pointer math doesn't land inside body (shouldn't happen).
        let abs_range = literal_range(body, raw_val, start).unwrap_or(stmt.range);
        out.push(Diagnostic {
          code: "sql039",
          severity: Severity::Error,
          message: format!(
            "INSERT value {} doesn't match column `{}` type `{}`",
            kind_name(lit),
            col_name,
            col.data_type
          ),
          range: abs_range,
        });
      }
    }
  }
}

fn literal_range(body: &str, lit: &str, stmt_start: usize) -> Option<text_size::TextRange> {
  let body_addr = body.as_ptr() as usize;
  let lit_addr = lit.as_ptr() as usize;
  if lit_addr < body_addr {
    return None;
  }
  let rel = lit_addr - body_addr;
  if rel + lit.len() > body.len() {
    return None;
  }
  // Trim leading whitespace inside the slice so the range hugs the
  // literal text, not the surrounding spaces from the comma split.
  let lead_ws = lit.len() - lit.trim_start().len();
  let trail_ws = lit.len() - lit.trim_end().len();
  let abs_s = stmt_start + rel + lead_ws;
  let abs_e = stmt_start + rel + lit.len() - trail_ws;
  Some(text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()))
}

fn classify_literal(s: &str) -> LitKind {
  let t = s.trim();
  if t.is_empty() {
    return LitKind::Unknown;
  }
  let upper = t.to_ascii_uppercase();
  if upper == "NULL" {
    return LitKind::Null;
  }
  if upper == "TRUE" || upper == "FALSE" {
    return LitKind::Bool;
  }
  if t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2 {
    return LitKind::Str;
  }
  if t.chars().all(|c| c.is_ascii_digit() || c == '-') {
    return LitKind::Int;
  }
  if t.contains('.') && t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') {
    return LitKind::Float;
  }
  LitKind::Unknown
}

fn kind_name(k: LitKind) -> &'static str {
  match k {
    LitKind::Str => "text/string",
    LitKind::Int => "integer",
    LitKind::Float => "numeric",
    LitKind::Bool => "boolean",
    LitKind::Null => "null",
    LitKind::Unknown => "unknown",
  }
}

fn compatible(kind: LitKind, declared: &str) -> bool {
  let d = declared.to_ascii_uppercase();
  // Strip the optional length spec `(...)` and any schema prefix
  // (`pg_catalog.varchar` -> `VARCHAR`). The catalog introspection
  // returns fully qualified type names for built-ins.
  let d = d.split('(').next().unwrap_or(&d).trim();
  let d = d.rsplit('.').next().unwrap_or(d).trim();
  let int_types =
    ["INT", "INTEGER", "BIGINT", "SMALLINT", "INT4", "INT8", "INT2", "SERIAL", "BIGSERIAL", "SMALLSERIAL"];
  let num_types = ["NUMERIC", "DECIMAL", "REAL", "DOUBLE", "FLOAT", "MONEY"];
  let str_types = ["TEXT", "VARCHAR", "CHAR", "CHARACTER", "CITEXT", "NAME"];
  let uuid_types = ["UUID"];
  let bool_types = ["BOOLEAN", "BOOL"];
  let time_types = ["DATE", "TIMESTAMP", "TIMESTAMPTZ", "TIME", "INTERVAL"];
  match kind {
    LitKind::Str => {
      str_types.iter().any(|t| d.starts_with(t))
        || uuid_types.iter().any(|t| d == *t)
        || time_types.iter().any(|t| d.starts_with(t))
    },
    LitKind::Int => int_types.iter().any(|t| d == *t) || num_types.iter().any(|t| d.starts_with(t)),
    LitKind::Float => num_types.iter().any(|t| d.starts_with(t)),
    LitKind::Bool => bool_types.iter().any(|t| d == *t),
    _ => true,
  }
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = open;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn split_top_level_commas(s: &str) -> Vec<String> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut start = 0;
  let mut depth = 0i32;
  let mut i = 0;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => {
        out.push(s[start..i].to_string());
        start = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  out.push(s[start..].to_string());
  out
}
