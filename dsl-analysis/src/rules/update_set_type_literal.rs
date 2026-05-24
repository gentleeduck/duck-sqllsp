//! sql171: `UPDATE t SET <col> = <literal>` where the literal kind
//! disagrees with the column's catalog type. Mirror of sql039 for
//! the SET assignment path.
//!
//! Conservative: only literal kinds we can classify with high
//! confidence (string / integer / float / boolean / NULL). Function
//! calls, expressions, casts, subqueries -> skipped.

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
    "sql171"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Update(u) = &stmt.kind else { return };
    let Some(t) = catalog.find_table(u.table.schema.as_deref(), &u.table.name) else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(set_at) = upper.find(" SET ") else { return };
    let set_start = set_at + 5;
    let set_end = ["WHERE", "RETURNING", "FROM"]
      .iter()
      .filter_map(|kw| upper[set_start..].find(&format!(" {kw} ")).map(|i| set_start + i))
      .min()
      .unwrap_or(body.len());
    let set_clause = &body[set_start..set_end];

    for entry in split_top_commas(set_clause) {
      let item = entry.trim();
      if item.is_empty() {
        continue;
      }
      let Some(eq_at) = item.find('=') else { continue };
      let lhs = item[..eq_at].trim();
      let rhs = item[eq_at + 1..].trim();
      if lhs.is_empty() || rhs.is_empty() {
        continue;
      }
      let col_name = lhs.split('.').next_back().unwrap_or(lhs).trim_matches('"');
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
      let lit = classify_literal(rhs);
      if matches!(lit, LitKind::Unknown | LitKind::Null) {
        continue;
      }
      if !compatible(lit, &col.data_type) {
        // Compute the rhs's absolute byte range.
        let rel_start = entry.as_ptr() as usize - body.as_ptr() as usize;
        let entry_off_in_body = rel_start;
        let rhs_off_in_entry = entry.find('=').map(|i| i + 1).unwrap_or(0);
        let rhs_trim_lead = entry[rhs_off_in_entry..].len() - entry[rhs_off_in_entry..].trim_start().len();
        let abs_s = start + entry_off_in_body + rhs_off_in_entry + rhs_trim_lead;
        let abs_e = abs_s + rhs.len();
        out.push(Diagnostic {
          code: "sql171",
          severity: Severity::Error,
          message: format!(
            "UPDATE SET value {} doesn't match column `{}` type `{}`",
            kind_name(lit),
            col.name,
            col.data_type
          ),
          range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
    }
  }
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
  if t.starts_with('\'') {
    return LitKind::Str;
  }
  let body = if t.starts_with('-') || t.starts_with('+') { &t[1..] } else { t };
  if !body.is_empty() && body.chars().all(|c| c.is_ascii_digit()) {
    return LitKind::Int;
  }
  if !body.is_empty() && body.chars().all(|c| c.is_ascii_digit() || c == '.') && body.contains('.') {
    return LitKind::Float;
  }
  LitKind::Unknown
}

fn kind_name(k: LitKind) -> &'static str {
  match k {
    LitKind::Str => "text/string",
    LitKind::Int => "integer",
    LitKind::Float => "float",
    LitKind::Bool => "boolean",
    _ => "?",
  }
}

fn compatible(kind: LitKind, declared: &str) -> bool {
  let d = declared.to_ascii_uppercase();
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
    }
    LitKind::Int => int_types.iter().any(|t| d == *t) || num_types.iter().any(|t| d.starts_with(t)),
    LitKind::Float => num_types.iter().any(|t| d.starts_with(t)),
    LitKind::Bool => bool_types.iter().any(|t| d == *t),
    _ => true,
  }
}

fn split_top_commas(s: &str) -> Vec<&str> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < n {
    match bytes[i] {
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      }
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(&s[start..i]);
        start = i + 1;
      }
      _ => {}
    }
    i += 1;
  }
  if start < n {
    out.push(&s[start..]);
  }
  out
}
