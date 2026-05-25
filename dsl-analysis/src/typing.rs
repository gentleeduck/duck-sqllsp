//! Lightweight type-family classifier for expression-level rules.
//!
//! This is intentionally not a real type system -- it groups PG built-in
//! types into a small set of compatibility families that most static
//! checks care about: numeric vs text vs bool vs date/time vs bytea vs
//! json vs uuid vs array. Catalog-typed columns and SQL literals are the
//! two inputs; everything else returns `Unknown` and the caller must
//! decide whether to bail.

use dsl_catalog::Catalog;
use dsl_resolve::Scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeFamily {
  Int,
  Numeric,
  Text,
  Bool,
  Json,
  Uuid,
  Date,
  Time,
  Timestamp,
  Interval,
  Bytea,
  Array,
  Unknown,
}

impl TypeFamily {
  pub fn is_numeric(self) -> bool {
    matches!(self, TypeFamily::Int | TypeFamily::Numeric)
  }
  pub fn name(self) -> &'static str {
    match self {
      TypeFamily::Int => "integer",
      TypeFamily::Numeric => "numeric",
      TypeFamily::Text => "text",
      TypeFamily::Bool => "boolean",
      TypeFamily::Json => "json",
      TypeFamily::Uuid => "uuid",
      TypeFamily::Date => "date",
      TypeFamily::Time => "time",
      TypeFamily::Timestamp => "timestamp",
      TypeFamily::Interval => "interval",
      TypeFamily::Bytea => "bytea",
      TypeFamily::Array => "array",
      TypeFamily::Unknown => "unknown",
    }
  }
}

/// Map a PG data-type string (catalog-shaped or user-written) to a family.
/// Strips `pg_catalog.` prefix, length modifiers (`(...)`), array suffix
/// (`[]`), and case folds.
pub fn classify(data_type: &str) -> TypeFamily {
  let s = data_type.trim();
  if s.ends_with("[]") || s.to_ascii_uppercase().starts_with("ARRAY") {
    return TypeFamily::Array;
  }
  let s = s.rsplit('.').next().unwrap_or(s);
  let mut s = s.to_ascii_lowercase();
  if let Some(paren) = s.find('(') {
    s.truncate(paren);
  }
  let s = s.trim();
  match s {
    "int" | "int2" | "int4" | "int8" | "integer" | "smallint" | "bigint" | "serial" | "smallserial" | "bigserial" => TypeFamily::Int,
    "numeric" | "decimal" | "real" | "double precision" | "double" | "float" | "float4" | "float8" => TypeFamily::Numeric,
    "text" | "varchar" | "character varying" | "character" | "char" | "bpchar" | "citext" | "name" => TypeFamily::Text,
    "bool" | "boolean" => TypeFamily::Bool,
    "json" | "jsonb" => TypeFamily::Json,
    "uuid" => TypeFamily::Uuid,
    "date" => TypeFamily::Date,
    "time" | "timetz" | "time with time zone" | "time without time zone" => TypeFamily::Time,
    "timestamp" | "timestamptz" | "timestamp with time zone" | "timestamp without time zone" => TypeFamily::Timestamp,
    "interval" => TypeFamily::Interval,
    "bytea" => TypeFamily::Bytea,
    _ => TypeFamily::Unknown,
  }
}

/// Guess a literal's family from its source text. Bare identifiers
/// return `Unknown` (the caller should run column lookup instead).
pub fn literal_family(text: &str) -> TypeFamily {
  let t = text.trim();
  if t.is_empty() { return TypeFamily::Unknown }
  let upper = t.to_ascii_uppercase();
  if upper == "NULL" { return TypeFamily::Unknown }
  if upper == "TRUE" || upper == "FALSE" { return TypeFamily::Bool }
  if t.starts_with('\'') && t.ends_with('\'') { return TypeFamily::Text }
  if t.starts_with("E'") || t.starts_with("e'") { return TypeFamily::Text }
  if (t.starts_with("ARRAY[") || t.starts_with("array[")) && t.ends_with(']') { return TypeFamily::Array }
  let bytes = t.as_bytes();
  if !bytes.is_empty() && (bytes[0].is_ascii_digit() || bytes[0] == b'-' || bytes[0] == b'+') {
    let has_dot = t.contains('.') || t.contains('e') || t.contains('E');
    return if has_dot { TypeFamily::Numeric } else { TypeFamily::Int };
  }
  // CAST(... AS T) / x::T -- pull the trailing type.
  if let Some(at) = t.rfind("::") {
    return classify(&t[at + 2..]);
  }
  let upper_t = t.to_ascii_uppercase();
  if upper_t.starts_with("CAST(") && upper_t.ends_with(')') {
    if let Some(as_at) = upper_t.rfind(" AS ") {
      let inner = &t[as_at + 4..t.len() - 1];
      return classify(inner);
    }
  }
  TypeFamily::Unknown
}

/// Resolve `qualifier.col` (or bare `col`) to its catalog column family.
/// Walks `scope.tables()` for the qualifier, then matches column name
/// against that table's catalog row. Falls back to a single matching
/// column across all bound tables when `qualifier` is None.
pub fn column_family(scope: &Scope, catalog: &Catalog, qualifier: Option<&str>, col: &str) -> Option<TypeFamily> {
  let cat_col = lookup_column(scope, catalog, qualifier, col)?;
  Some(classify(&cat_col.data_type))
}

/// Returns Some(true) when the column is known-nullable, Some(false)
/// when known NOT NULL, None when the lookup fails.
pub fn column_nullable(scope: &Scope, catalog: &Catalog, qualifier: Option<&str>, col: &str) -> Option<bool> {
  let cat_col = lookup_column(scope, catalog, qualifier, col)?;
  Some(cat_col.nullable)
}

fn lookup_column<'a>(scope: &Scope, catalog: &'a Catalog, qualifier: Option<&str>, col: &str) -> Option<&'a dsl_catalog::Column> {
  if let Some(q) = qualifier {
    let binding = scope.get(q)?;
    let table = catalog.find_table(binding.table.schema.as_deref(), &binding.table.name)?;
    return table.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col));
  }
  let mut hit: Option<&'a dsl_catalog::Column> = None;
  for b in scope.tables() {
    let Some(table) = catalog.find_table(b.table.schema.as_deref(), &b.table.name) else { continue };
    if let Some(c) = table.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col)) {
      if hit.is_some() { return None }
      hit = Some(c);
    }
  }
  hit
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn classify_int_aliases() {
    assert_eq!(classify("int"), TypeFamily::Int);
    assert_eq!(classify("INTEGER"), TypeFamily::Int);
    assert_eq!(classify("pg_catalog.int4"), TypeFamily::Int);
    assert_eq!(classify("bigserial"), TypeFamily::Int);
  }

  #[test]
  fn classify_text_aliases() {
    assert_eq!(classify("text"), TypeFamily::Text);
    assert_eq!(classify("varchar(255)"), TypeFamily::Text);
    assert_eq!(classify("character varying"), TypeFamily::Text);
    assert_eq!(classify("CITEXT"), TypeFamily::Text);
  }

  #[test]
  fn classify_array_suffix() {
    assert_eq!(classify("int[]"), TypeFamily::Array);
    assert_eq!(classify("text[]"), TypeFamily::Array);
  }

  #[test]
  fn literal_family_basics() {
    assert_eq!(literal_family("'foo'"), TypeFamily::Text);
    assert_eq!(literal_family("42"), TypeFamily::Int);
    assert_eq!(literal_family("3.14"), TypeFamily::Numeric);
    assert_eq!(literal_family("TRUE"), TypeFamily::Bool);
    assert_eq!(literal_family("NULL"), TypeFamily::Unknown);
    assert_eq!(literal_family("'x'::uuid"), TypeFamily::Uuid);
    assert_eq!(literal_family("CAST('1' AS int)"), TypeFamily::Int);
  }
}
