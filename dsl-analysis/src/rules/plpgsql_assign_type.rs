//! sql170: `x := <lit>` inside a PL/pgSQL body where the literal kind
//! disagrees with x's declared type. Catches `DECLARE x INT; ... x :=
//! 'str';` and similar at edit time -- Postgres errors at execution.
//!
//! Conservative: only literal kinds we can classify with high
//! confidence (string / integer / float / boolean / NULL); skips
//! function calls / expressions / casts.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
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
    "sql170"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    // Only fire inside a dollar-quoted body (PL/pgSQL or SQL fn).
    let dollar_start = match body.find("$$") {
      Some(at) => at + 2,
      None => return,
    };
    let dollar_end = body[dollar_start..].find("$$").map(|i| dollar_start + i).unwrap_or(body.len());
    let block = &body[dollar_start..dollar_end];
    let mut declares: std::collections::HashMap<String, String> = Default::default();
    collect_declares(block, &mut declares);
    // Each `<name> := <expr>;` -- check the rhs.
    let bytes = block.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i < n {
      if !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
        i += 1;
        continue;
      }
      let name_start = i;
      while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
      }
      let name = block[name_start..i].to_string();
      let mut k = i;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k + 1 < n && bytes[k] == b':' && bytes[k + 1] == b'=' {
        let rhs_start = k + 2;
        let rhs_end = block[rhs_start..].find(';').map(|i| rhs_start + i).unwrap_or(n);
        let rhs = block[rhs_start..rhs_end].trim();
        let key = name.to_ascii_lowercase();
        if let Some(decl_type) = declares.get(&key) {
          let lit = classify_literal(rhs);
          if matches!(lit, LitKind::Unknown | LitKind::Null) {
            i = rhs_end + 1;
            continue;
          }
          if !compatible(lit, decl_type) {
            let abs_s = start + dollar_start + rhs_start + (rhs_start - block[rhs_start..].find(rhs.chars().next().unwrap_or(' ')).unwrap_or(0) - rhs_start);
            // simpler: point at rhs span
            let abs_lit_s = start + dollar_start + rhs_start
              + block[rhs_start..rhs_end].find(rhs.chars().next().unwrap_or(' ')).unwrap_or(0);
            let abs_lit_e = abs_lit_s + rhs.len();
            out.push(Diagnostic {
              code: "sql170",
              severity: Severity::Error,
              message: format!(
                "PL/pgSQL assignment: value {} doesn't match `{}`'s declared type `{}`",
                kind_name(lit),
                name,
                decl_type
              ),
              range: text_size::TextRange::new((abs_lit_s as u32).into(), (abs_lit_e as u32).into()),
            });
            let _ = abs_s;
          }
        }
        i = rhs_end + 1;
      }
    }
  }
}

/// Walk the DECLARE section of a PL/pgSQL block; populate `out` with
/// `name -> declared_type` (lowercased name). Stops at BEGIN.
fn collect_declares(block: &str, out: &mut std::collections::HashMap<String, String>) {
  let upper = block.to_ascii_uppercase();
  let Some(decl_at) = upper.find("DECLARE") else { return };
  let body = &block[decl_at + "DECLARE".len()..];
  let body_upper = upper[decl_at + "DECLARE".len()..].to_string();
  let begin_at = body_upper.find("BEGIN").unwrap_or(body.len());
  let section = &body[..begin_at];
  for line in section.split(';') {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }
    let mut parts = trimmed.split_whitespace();
    let Some(name) = parts.next() else { continue };
    let Some(type_name) = parts.next() else { continue };
    let name = name.trim_matches('"').to_ascii_lowercase();
    let type_name = type_name.trim_end_matches(',').trim_end_matches(';').to_string();
    out.insert(name, type_name);
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
  if t.starts_with('-') || t.starts_with('+') || t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
    if t.contains('.') {
      if t[1..].chars().all(|c| c.is_ascii_digit() || c == '.') {
        return LitKind::Float;
      }
    } else if t[(if t.starts_with('-') || t.starts_with('+') { 1 } else { 0 })..].chars().all(|c| c.is_ascii_digit()) {
      return LitKind::Int;
    }
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
