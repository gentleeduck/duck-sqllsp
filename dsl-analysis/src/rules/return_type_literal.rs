//! sql031: `RETURN <literal>` type doesn't match declared `RETURNS <type>`.
//!
//! Catches the easy literal-return mismatches:
//!   - `RETURN 'string';` in `RETURNS INT`        -> Error
//!   - `RETURN 1;`        in `RETURNS TEXT`       -> Error
//!   - `RETURN true;`     in `RETURNS INT`        -> Error
//!
//! Skips when the return value is anything other than a bare literal
//! (column, expression, function call) -- those need real type
//! inference, deferred to a follow-up rule.

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
    "sql031"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Unknown { .. }) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") {
      return;
    }
    let Some(declared) = find_returns_type(&upper) else { return };
    let Some(body_text) = dollar_body(body) else { return };
    let upper_body = body_text.to_ascii_uppercase();
    let stripped = strip_comments(&upper_body);

    // Walk each `RETURN <value>;` at top level. Classify the value
    // as a literal kind; flag if incompatible.
    let bytes = stripped.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 6 <= n {
      if &stripped[i..i + 6] == "RETURN" {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let next_ok = i + 6 == n || !is_word(bytes[i + 6] as char);
        if prev_ok && next_ok {
          let mut j = i + 6;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          // Read until `;`.
          let val_start = j;
          while j < n && bytes[j] != b';' {
            j += 1;
          }
          let raw_val = body_text[val_start..j.min(body_text.len())].trim();
          if let Some(kind) = classify_literal(raw_val)
            && !compatible(kind, declared.as_str())
          {
            let base = source.find(body_text).unwrap_or(start);
            let abs_start = base + i;
            let abs_end = base + j;
            out.push(Diagnostic {
              code: "sql031",
              severity: Severity::Error,
              message: format!("RETURN value type {} doesn't match declared RETURNS {}", kind_name(kind), declared),
              range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
            });
          }
        }
        i += 6;
      } else {
        i += 1;
      }
    }
  }
}

fn classify_literal(s: &str) -> Option<LitKind> {
  let t = s.trim_end_matches(';').trim();
  if t.is_empty() {
    return None;
  }
  let upper = t.to_ascii_uppercase();
  if upper == "NULL" {
    return Some(LitKind::Null);
  }
  if upper == "TRUE" || upper == "FALSE" {
    return Some(LitKind::Bool);
  }
  if t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2 {
    return Some(LitKind::Str);
  }
  if t.chars().all(|c| c.is_ascii_digit() || c == '-') && !t.is_empty() {
    return Some(LitKind::Int);
  }
  if t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') && t.contains('.') {
    return Some(LitKind::Float);
  }
  // Anything else (expression, identifier, call) -> not a literal.
  Some(LitKind::Unknown)
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

fn compatible(kind: LitKind, declared_upper: &str) -> bool {
  // Unknown / NULL are silent -- can't classify safely.
  if matches!(kind, LitKind::Unknown | LitKind::Null) {
    return true;
  }
  let d = declared_upper.trim_end_matches(';').trim();
  // Strip size qualifier: VARCHAR(255) -> VARCHAR.
  let d = d.split('(').next().unwrap_or(d).trim();
  let int_types =
    ["INT", "INTEGER", "BIGINT", "SMALLINT", "INT4", "INT8", "INT2", "SERIAL", "BIGSERIAL", "SMALLSERIAL"];
  let num_types = ["NUMERIC", "DECIMAL", "REAL", "DOUBLE", "FLOAT", "MONEY"];
  let str_types = ["TEXT", "VARCHAR", "CHAR", "CHARACTER", "CITEXT", "NAME", "UUID"];
  let bool_types = ["BOOLEAN", "BOOL"];
  match kind {
    LitKind::Str => str_types.iter().any(|t| d.starts_with(t)),
    LitKind::Int => int_types.contains(&d) || num_types.iter().any(|t| d.starts_with(t)),
    LitKind::Float => num_types.iter().any(|t| d.starts_with(t)),
    LitKind::Bool => bool_types.contains(&d),
    _ => true,
  }
}

fn find_returns_type(upper: &str) -> Option<String> {
  let needle = "RETURNS";
  let idx = upper.find(needle)?;
  let after = idx + needle.len();
  let rest = &upper[after..];
  let trimmed = rest.trim_start();
  // Read until whitespace, `(`, or end.
  let mut out = String::new();
  let mut paren = 0i32;
  for c in trimmed.chars() {
    if paren == 0 && c.is_whitespace() {
      break;
    }
    if c == '(' {
      paren += 1;
      out.push(c);
      continue;
    }
    if c == ')' {
      paren -= 1;
      out.push(c);
      continue;
    }
    out.push(c);
  }
  if out.is_empty() { None } else { Some(out) }
}

fn dollar_body(text: &str) -> Option<&str> {
  let start = text.find("$$")?;
  let after = start + 2;
  let end_rel = text[after..].find("$$")?;
  Some(&text[after..after + end_rel])
}

fn strip_comments(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut i = 0;
  while i < n {
    if i + 1 < n && bytes[i] == b'-' && bytes[i + 1] == b'-' {
      while i < n && bytes[i] != b'\n' {
        i += 1;
      }
    } else if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
      i += 2;
      while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
        i += 1;
      }
      i = (i + 2).min(n);
    } else {
      out.push(bytes[i] as char);
      i += 1;
    }
  }
  out
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
