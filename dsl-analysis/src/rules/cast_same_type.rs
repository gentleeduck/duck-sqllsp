//! sql415: `col::T` or `CAST(col AS T)` where T is the column's
//! catalog data type -- the cast is a no-op and adds visual noise
//! (and sometimes hides the wrong type from review). Drop the cast.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql415"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if scope.is_empty() || catalog.tables().next().is_none() {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let bytes = cleaned.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<String> = std::collections::HashSet::new();
    while i + 1 < n {
      // `::` form
      if bytes[i] == b':' && bytes[i + 1] == b':' {
        if let Some((col_text, type_text, abs_end_in_clean)) = parse_double_colon_cast(&cleaned, i) {
          maybe_emit(scope, catalog, &col_text, &type_text, i, abs_end_in_clean, start, &mut emitted, out);
        }
        i += 2;
        continue;
      }
      // `CAST(` form
      if i + 5 <= n
        && cleaned[i..i + 5].eq_ignore_ascii_case("CAST(")
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && let Some((col_text, type_text, abs_end_in_clean)) = parse_cast_call(&cleaned, i)
      {
        maybe_emit(scope, catalog, &col_text, &type_text, i, abs_end_in_clean, start, &mut emitted, out);
      }
      i += 1;
    }
  }
}

fn parse_double_colon_cast(s: &str, dcolon_at: usize) -> Option<(String, String, usize)> {
  let bytes = s.as_bytes();
  // Read column ident to the left (allow dotted qualifier).
  let mut left_end = dcolon_at;
  while left_end > 0 && bytes[left_end - 1].is_ascii_whitespace() {
    left_end -= 1;
  }
  if left_end == 0 || !is_ident_byte(bytes[left_end - 1]) {
    return None;
  }
  let mut left_start = left_end;
  while left_start > 0 {
    let b = bytes[left_start - 1];
    if is_ident_byte(b) || b == b'.' {
      left_start -= 1;
    } else {
      break;
    }
  }
  let col = s[left_start..left_end].to_string();
  // Read type to the right (single word).
  let mut right = dcolon_at + 2;
  while right < bytes.len() && bytes[right].is_ascii_whitespace() {
    right += 1;
  }
  let type_start = right;
  while right < bytes.len() && is_ident_byte(bytes[right]) {
    right += 1;
  }
  if type_start == right {
    return None;
  }
  let ty = s[type_start..right].to_string();
  Some((col, ty, right))
}

fn parse_cast_call(s: &str, cast_at: usize) -> Option<(String, String, usize)> {
  let bytes = s.as_bytes();
  // Inside the paren: `<expr> AS <type>` -- match the same shape we
  // handle for `::` (column expr only, single-word type).
  let mut i = cast_at + 5;
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  // Read column ident (allow qualifier).
  let col_start = i;
  if i >= bytes.len() || !is_ident_byte(bytes[i]) {
    return None;
  }
  while i < bytes.len() && (is_ident_byte(bytes[i]) || bytes[i] == b'.') {
    i += 1;
  }
  let col = s[col_start..i].to_string();
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  // Match AS.
  if i + 2 > bytes.len() || !s[i..i + 2].eq_ignore_ascii_case("AS") {
    return None;
  }
  i += 2;
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  let ty_start = i;
  while i < bytes.len() && is_ident_byte(bytes[i]) {
    i += 1;
  }
  if ty_start == i {
    return None;
  }
  let ty = s[ty_start..i].to_string();
  // Verify closing `)` after optional whitespace.
  let mut j = i;
  while j < bytes.len() && bytes[j].is_ascii_whitespace() {
    j += 1;
  }
  if j >= bytes.len() || bytes[j] != b')' {
    return None;
  }
  Some((col, ty, j + 1))
}

#[allow(clippy::too_many_arguments)]
fn maybe_emit(
  scope: &Scope,
  catalog: &Catalog,
  col_text: &str,
  type_text: &str,
  abs_start_in_clean: usize,
  abs_end_in_clean: usize,
  stmt_start: usize,
  emitted: &mut std::collections::HashSet<String>,
  out: &mut Vec<Diagnostic>,
) {
  // Skip numeric LHS / function-call LHS / etc -- only bare column or
  // qualifier.column.
  let (qualifier, name) = if let Some((q, n)) = col_text.split_once('.') {
    (Some(q.to_string()), n.to_string())
  } else {
    (None, col_text.to_string())
  };
  if name.is_empty() || name.chars().any(|c| !(c.is_alphanumeric() || c == '_')) {
    return;
  }
  let Some(col_type) = lookup_column_type(scope, catalog, qualifier.as_deref(), &name) else {
    return;
  };
  if !type_equivalent(&col_type, type_text) {
    return;
  }
  let key = format!("{}:{}:{}", abs_start_in_clean, name, type_text.to_ascii_lowercase());
  if !emitted.insert(key) {
    return;
  }
  let display = match &qualifier {
    Some(q) => format!("{q}.{name}"),
    None => name.clone(),
  };
  out.push(Diagnostic {
    code: "sql415",
    severity: Severity::Hint,
    message: format!("cast of `{display}` to `{type_text}` is a no-op -- column is already `{col_type}`; drop the cast"),
    range: TextRange::new(
      ((stmt_start + abs_start_in_clean) as u32).into(),
      ((stmt_start + abs_end_in_clean) as u32).into(),
    ),
  });
}

fn lookup_column_type(scope: &Scope, catalog: &Catalog, qualifier: Option<&str>, name: &str) -> Option<String> {
  // Qualified: resolve via the binding's underlying table.
  if let Some(q) = qualifier {
    if let Some((schema, table)) = q.split_once('.')
      && let Some(t) = catalog.find_table(Some(schema), table)
    {
      return t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(name)).map(|c| c.data_type.clone());
    }
    if let Some(b) = scope.get(q)
      && let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name)
    {
      return t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(name)).map(|c| c.data_type.clone());
    }
    return None;
  }
  // Unqualified: walk in-scope catalog tables; if exactly one match,
  // return its type. Multiple matches -> ambiguous, skip.
  let mut hit: Option<String> = None;
  for b in scope.tables() {
    let Some(t) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) else {
      continue;
    };
    for c in &t.columns {
      if c.name.eq_ignore_ascii_case(name) {
        if hit.is_some() {
          return None;
        }
        hit = Some(c.data_type.clone());
      }
    }
  }
  hit
}

/// Compare two PG type names case-insensitively, with a few common
/// aliases collapsed (int = integer = int4, varchar = character
/// varying, etc.). Conservative -- when in doubt return false so the
/// rule stays quiet rather than fire a false positive.
fn type_equivalent(a: &str, b: &str) -> bool {
  fn canon(t: &str) -> String {
    let lower = t.trim().to_ascii_lowercase();
    // Strip any precision/length suffix like `(255)` for the common
    // varchar / numeric / text variants.
    let bare = lower.split('(').next().unwrap_or(&lower).trim().to_string();
    match bare.as_str() {
      "int" | "int4" | "integer" => "integer".into(),
      "int2" | "smallint" => "smallint".into(),
      "int8" | "bigint" => "bigint".into(),
      "bool" | "boolean" => "boolean".into(),
      "float4" | "real" => "real".into(),
      "float8" | "double precision" => "double precision".into(),
      "varchar" | "character varying" => "varchar".into(),
      "char" | "character" | "bpchar" => "char".into(),
      _ => bare,
    }
  }
  canon(a) == canon(b)
}

fn is_ident_byte(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
