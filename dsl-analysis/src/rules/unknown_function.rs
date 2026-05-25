//! sql348: function call whose name isn't in the live catalog, the
//! built-in dsl-knowledge function table, or a buffer-local CREATE
//! FUNCTION. Helps catch typos and missing schema-qualified imports.
//!
//! Conservative -- skips anything that looks like a keyword form,
//! a CTE name reference, an explicit cast, or a method-style suffix
//! call that isn't actually a function (e.g. `count(*)`).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use std::collections::HashSet;

pub struct Rule;

/// Tokens that look like function calls but are SQL syntax.
const KEYWORDS: &[&str] = &[
  "CAST", "COALESCE", "NULLIF", "GREATEST", "LEAST", "EXTRACT", "OVERLAY",
  "POSITION", "SUBSTRING", "TRIM", "EXISTS", "ARRAY", "ROW", "VALUES",
  "IF", "CASE", "WHEN", "ANY", "ALL", "SOME", "IN", "BETWEEN", "LIKE",
  "ILIKE", "SIMILAR", "INTERVAL", "DATE", "TIME", "TIMESTAMP", "TIMESTAMPTZ",
  "NUMERIC", "DECIMAL", "INTEGER", "BIGINT", "SMALLINT", "REAL", "FLOAT",
  "TEXT", "VARCHAR", "CHAR", "BOOLEAN", "BOOL", "UUID", "JSON", "JSONB",
  "FILTER", "OVER", "WITHIN", "USING", "RETURNING", "WITH", "RECURSIVE",
  "ON", "AS", "AND", "OR", "NOT", "DISTINCT", "FROM", "WHERE", "GROUP",
  "ORDER", "HAVING", "LIMIT", "OFFSET", "FETCH", "FOR", "INNER", "OUTER",
  "LEFT", "RIGHT", "FULL", "CROSS", "LATERAL", "NATURAL", "JOIN", "UNION",
  "INTERSECT", "EXCEPT", "PARTITION", "WINDOW", "RANGE", "ROWS", "GROUPS",
  "PRECEDING", "FOLLOWING", "UNBOUNDED", "CURRENT", "FIRST", "LAST", "NULLS",
  "FUNCTION", "PROCEDURE", "TRIGGER", "TABLE", "INDEX", "VIEW", "POLICY",
  "TRUE", "FALSE", "NULL", "DEFAULT", "PRIMARY", "REFERENCES", "UNIQUE",
  "CHECK", "FOREIGN", "KEY", "CONSTRAINT", "CASCADE", "RESTRICT", "RESTART",
  "SET", "OF", "TO", "BY", "INTO", "RETURN", "BEGIN", "END", "RAISE",
  "NOTICE", "EXCEPTION", "WARNING", "DEBUG", "INFO", "LOG", "PERFORM",
  "DO", "LANGUAGE", "PLPGSQL", "SQL", "STABLE", "IMMUTABLE", "VOLATILE",
  "SECURITY", "DEFINER", "INVOKER", "STRICT", "PARALLEL", "SAFE", "UNSAFE",
  "RESTRICTED", "LEAKPROOF", "COST", "CALL",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql348"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let known = build_known_set(source, catalog);
    // Build a set of byte offsets that sit inside table/column/constraint
    // definition syntax. Identifiers landing in those positions are
    // not function calls -- `CREATE TABLE users (...)` and
    // `REFERENCES users(id)` and `CONSTRAINT u UNIQUE (col)` all
    // have `<ident>(` shapes that look like calls but aren't.
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
      // Skip string literals.
      if bytes[i] == b'\'' {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
        if i < bytes.len() { i += 1 }
        continue;
      }
      // Skip `--` line comments.
      if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
        while i < bytes.len() && bytes[i] != b'\n' { i += 1 }
        continue;
      }
      // Skip dollar-quoted string bodies.
      if bytes[i] == b'$' {
        if let Some(end_tag) = find_dollar_close(body, i) {
          i = end_tag;
          continue;
        }
      }
      // Identifier start.
      if !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') { i += 1; continue }
      let id_start = i;
      while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') { i += 1 }
      let id_end = i;
      // Allow optional schema.fn.
      let mut full_end = id_end;
      if i < bytes.len() && bytes[i] == b'.' {
        let after_dot = i + 1;
        let mut k = after_dot;
        while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') { k += 1 }
        if k > after_dot { full_end = k; i = k; }
      }
      // Must be followed by `(`, possibly with whitespace between.
      let mut j = i;
      while j < bytes.len() && bytes[j].is_ascii_whitespace() { j += 1 }
      if j >= bytes.len() || bytes[j] != b'(' { continue }
      // Pull the bare function name (after any schema qualifier).
      let full = &body[id_start..full_end];
      let bare = full.rsplit('.').next().unwrap_or(full);
      let upper = bare.to_ascii_uppercase();
      if KEYWORDS.contains(&upper.as_str()) { continue }
      // Skip when the preceding token is a DDL keyword that introduces
      // a name slot (CREATE TABLE foo, REFERENCES bar, INSERT INTO baz,
      // ALTER TABLE, etc). These syntactically match the function-call
      // shape `<ident>(` but aren't calls.
      let prev_word = preceding_word(body, id_start);
      let prev_upper = prev_word.to_ascii_uppercase();
      // FUNCTION / PROCEDURE are blocklisted because CREATE / DROP /
      // ALTER FUNCTION put the next ident in a name slot. But
      // `EXECUTE FUNCTION fn()` and `CALL fn()` are real calls --
      // detect via the word BEFORE the blocklisted keyword.
      if matches!(prev_upper.as_str(), "FUNCTION" | "PROCEDURE") {
        let prev_prev = preceding_word_before(body, id_start, prev_word.len());
        let pp_upper = prev_prev.to_ascii_uppercase();
        if !matches!(pp_upper.as_str(), "EXECUTE" | "CALL" | "PERFORM") {
          continue;
        }
        // Fall through: this is a real call, validate it.
      } else if PRECEDING_BLOCKLIST.contains(&prev_upper.as_str()) {
        continue;
      }
      // Type-cast-style: `INT(x)` etc. -- already caught by KEYWORDS.
      // Method-style: name followed by `(*)` is COUNT-only; allow it.
      // Lookup.
      if known.contains(&upper) { continue }
      let abs_s = start + id_start;
      let abs_e = start + full_end;
      out.push(Diagnostic {
        code: "sql348",
        severity: Severity::Warning,
        message: format!("unknown function `{bare}` -- not in catalog, dsl-knowledge, or this buffer"),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}

fn build_known_set(body: &str, catalog: &Catalog) -> HashSet<String> {
  let mut set: HashSet<String> = HashSet::new();
  // dsl-knowledge built-ins.
  for (k, _) in dsl_knowledge::tables::functions() {
    set.insert(k.to_ascii_uppercase());
  }
  // Catalog functions.
  for f in &catalog.functions {
    set.insert(f.name.to_ascii_uppercase());
  }
  // Buffer-local CREATE FUNCTION names.
  let upper_body = body.to_ascii_uppercase();
  let bytes = body.as_bytes();
  for needle in ["CREATE OR REPLACE FUNCTION ", "CREATE FUNCTION ", "CREATE OR REPLACE PROCEDURE ", "CREATE PROCEDURE "] {
    let mut from = 0usize;
    while let Some(rel) = upper_body[from..].find(needle) {
      let at = from + rel + needle.len();
      let mut k = at;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1 }
      let name_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') {
        k += 1;
      }
      let name = body[name_start..k].rsplit('.').next().unwrap_or(&body[name_start..k]).trim_matches('"');
      if !name.is_empty() {
        set.insert(name.to_ascii_uppercase());
      }
      from = k;
    }
  }
  set
}

/// Words that put the NEXT identifier into a name slot rather than a
/// function-call position. `<word> <ident>(` is therefore a definition,
/// reference, or DDL clause -- not a function call we should validate.
const PRECEDING_BLOCKLIST: &[&str] = &[
  "TABLE", "INDEX", "VIEW", "TYPE", "DOMAIN", "SCHEMA", "EXTENSION",
  "TRIGGER", "POLICY", "SEQUENCE", "FUNCTION", "PROCEDURE", "ROLE", "USER",
  "GROUP", "DATABASE", "TABLESPACE", "OPERATOR", "CLASS",
  "ON", "REFERENCES", "INTO", "FROM", "JOIN", "UPDATE", "DELETE",
  "ALTER", "DROP", "RENAME", "COLUMN", "CONSTRAINT", "EXISTS",
  "CASCADE", "RESTRICT", "USING", "WITH", "OF", "TO", "AS",
  "UNIQUE", "PRIMARY", "FOREIGN", "KEY", "CHECK",
  "BEFORE", "AFTER",
];


/// Pull the word ending at byte `end` (exclusive). Skips whitespace,
/// punctuation, dots. Returns "" when there's no word boundary.
fn preceding_word(body: &str, end: usize) -> &str {
  let bytes = body.as_bytes();
  let mut i = end;
  while i > 0 && bytes[i - 1].is_ascii_whitespace() { i -= 1 }
  let word_end = i;
  while i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_') { i -= 1 }
  if i == word_end { return "" }
  &body[i..word_end]
}

/// Pull the word that immediately precedes the word ending at `end - prev_len`
/// (so we can look two tokens back without re-walking the whitespace).
fn preceding_word_before<'a>(body: &'a str, end: usize, prev_len: usize) -> &'a str {
  let bytes = body.as_bytes();
  // Reach the start of the previous word: skip ws ending at `end`, then
  // step back over prev_len chars, then ask preceding_word from there.
  let mut i = end;
  while i > 0 && bytes[i - 1].is_ascii_whitespace() { i -= 1 }
  if i < prev_len { return "" }
  preceding_word(body, i - prev_len)
}

fn find_dollar_close(body: &str, dollar_at: usize) -> Option<usize> {
  let bytes = body.as_bytes();
  let after = dollar_at + 1;
  let mut k = after;
  while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') { k += 1 }
  if k >= bytes.len() || bytes[k] != b'$' { return None }
  let tag = &body[dollar_at..=k];
  body[k + 1..].find(tag).map(|p| k + 1 + p + tag.len())
}
