//! Build a synthetic catalog from CREATE TABLE statements in the current
//! file (and a few text-scanned object families) so completion / hover /
//! validation work even offline -- before the DB knows about anything,
//! or when there is no DB connection configured at all.
//!
//! Offline-mode coverage:
//!   * Tables  -- AST: every CREATE TABLE in the parsed file.
//!   * Sequences -- text-scan: CREATE SEQUENCE <name>.
//!   * Types -- text-scan: CREATE TYPE <name> AS ENUM/DOMAIN/(...).
//!   * Extensions -- text-scan: CREATE EXTENSION [IF NOT EXISTS] <name>.
//!   * Functions -- text-scan: CREATE [OR REPLACE] FUNCTION <name>.
//!   * Roles -- text-scan: CREATE ROLE/USER/GROUP <name>; plus a
//!     fixed offline fallback set (postgres, pg_read_all_data,
//!     pg_write_all_data) so role-completion / role-hover work even
//!     in a brand-new file.
//!
//! This is purely additive: the live catalog from `dsl-catalog` and the
//! source-derived one are merged at lookup time. When both define the
//! same object, the live catalog wins because it has richer metadata
//! (constraints, indexes, comments).

use dsl_catalog::{
  CATALOG_VERSION, Catalog, Column, Constraint, ConstraintKind, ConstraintRef, Extension,
  Function, FunctionArg, IndexDef, Policy, Schema, Sequence, Table, TableKind, Trigger, Type,
  TypeKind,
};
use dsl_parse::{ParsedFile, StatementKind};

/// Build a [`Catalog`] from every CREATE TABLE statement in `file`.
/// Returns an empty catalog if none are found.
pub fn from_file(file: &ParsedFile) -> Catalog {
  let mut public = Schema { name: "public".into(), tables: Vec::new() };
  let mut other: std::collections::BTreeMap<String, Schema> = Default::default();

  for stmt in &file.statements {
    let StatementKind::CreateTable(ct) = &stmt.kind else {
      continue;
    };
    let schema_name = ct.table.schema.clone().unwrap_or_else(|| "public".into());
    let table = Table {
      schema: schema_name.clone(),
      name: ct.table.name.clone(),
      kind: TableKind::Table,
      columns: ct
        .columns
        .iter()
        .map(|c| Column {
          name: c.name.clone(),
          data_type: c.type_name.clone(),
          nullable: c.nullable,
          default: c.default.clone(),
          comment: None,
          generated: None,
          json_keys: None,
        })
        .collect(),
      constraints: Vec::new(),
      indexes: Vec::new(),
      triggers: Vec::new(),
      policies: Vec::new(),
      // No synthetic comment -- the table is real, not a doc artifact.
      // Real COMMENT ON TABLE statements get picked up in
      // scan_table_comments() below when present.
      comment: None,
      row_estimate: None,
    };
    if schema_name == "public" {
      public.tables.push(table);
    } else {
      other
        .entry(schema_name.clone())
        .or_insert_with(|| Schema { name: schema_name, tables: Vec::new() })
        .tables
        .push(table);
    }
  }

  let mut schemas = Vec::new();
  if !public.tables.is_empty() {
    schemas.push(public);
  }
  schemas.extend(other.into_values());

  Catalog {
    version: CATALOG_VERSION,
    connection_id: "<source>".into(),
    schemas,
    functions: Vec::new(),
    types: Vec::new(),
    roles: Vec::new(),
    sequences: Vec::new(),
    extensions: Vec::new(),
  }
}

/// Full offline-mode catalog: tables from AST + sequences / types /
/// extensions / functions / roles harvested from raw source text. The
/// scans are intentionally lenient -- partial / unparseable input is
/// fine because the goal is "give the user *something* without a DB".
/// `source` is the buffer text the AST in `file` came from.
pub fn from_source(file: &ParsedFile, source: &str) -> Catalog {
  let mut cat = from_file(file);
  cat.sequences = scan_sequences(source);
  cat.types = scan_types(source);
  cat.extensions = scan_extensions(source);
  cat.functions = scan_functions(source);
  cat.roles = scan_roles(source);
  // Add PARTITION OF child tables by copying the parent's columns.
  // The CREATE TABLE child PARTITION OF parent form has no inline
  // columns, so from_file misses them; downstream rules (sql002 etc)
  // then can't resolve `child.col`.
  inherit_partition_columns(&mut cat, source);
  // Walk every CREATE TABLE body and pull inline + table-level
  // PRIMARY KEY / UNIQUE / FOREIGN KEY constraints from the source.
  // The AST doesn't carry these (CreateTableStmt::columns only has
  // name + type + nullable + default), so this text-scan is how
  // offline mode learns about FKs at all.
  let comments_by_col = scan_column_comments(source);
  let table_comments = scan_table_comments(source);
  let indexes_by_table = scan_indexes(source);
  let triggers_by_table = scan_triggers(source);
  let policies_by_table = scan_policies(source);
  for schema in cat.schemas.iter_mut() {
    for table in schema.tables.iter_mut() {
      table.constraints = scan_constraints_for(source, &table.name);
      let key = table.name.to_ascii_lowercase();
      if let Some(c) = table_comments.get(&key) {
        table.comment = Some(c.clone());
      }
      if let Some(idxs) = indexes_by_table.get(&key) {
        table.indexes = idxs.clone();
      }
      if let Some(trgs) = triggers_by_table.get(&key) {
        table.triggers = trgs.clone();
      }
      if let Some(pols) = policies_by_table.get(&key) {
        table.policies = pols.clone();
      }
      let generated_by_col = scan_generated_for(source, &table.name);
      let json_keys_by_col = scan_json_keys_for(source, &table.name);
      for col in table.columns.iter_mut() {
        let ckey = format!("{}.{}", table.name.to_ascii_lowercase(), col.name.to_ascii_lowercase());
        if let Some(c) = comments_by_col.get(&ckey) {
          col.comment = Some(c.clone());
        }
        if let Some(g) = generated_by_col.get(&col.name.to_ascii_lowercase()) {
          col.generated = Some(g.clone());
        }
        if let Some(keys) = json_keys_by_col.get(&col.name.to_ascii_lowercase()) {
          col.json_keys = Some(keys.clone());
        }
      }
    }
  }
  cat
}

/// `CREATE [OR REPLACE] POLICY <name> ON <tbl>
///   [AS {PERMISSIVE|RESTRICTIVE}]
///   [FOR {ALL|SELECT|INSERT|UPDATE|DELETE}]
///   [TO <roles>]
///   [USING (<expr>)]
///   [WITH CHECK (<expr>)]`
/// -> map<table_lower, Vec<Policy>>.
/// `CREATE TABLE child PARTITION OF parent ...` declares `child`
/// inheriting parent's full column list. The AST converter only sees
/// the explicit `table_elts`, so the child lands in the catalog with
/// zero columns -- unknown-column rules can't resolve `child.col`.
/// Walk the source, find every PARTITION OF and copy parent columns
/// onto matching children already in the catalog.
fn inherit_partition_columns(cat: &mut Catalog, src: &str) {
  let cleaned = strip_string_literals(src);
  let upper = cleaned.to_ascii_uppercase();
  let bytes = cleaned.as_bytes();
  let n = bytes.len();
  let needle = "CREATE TABLE";
  let mut from = 0usize;
  let mut pairs: Vec<(String, String)> = Vec::new();
  while let Some(rel) = upper[from..].find(needle) {
    let at = from + rel;
    let mut k = at + needle.len();
    while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
    if upper[k..].starts_with("IF NOT EXISTS") {
      k += "IF NOT EXISTS".len();
      while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
    }
    let id_start = k;
    while k < n && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') { k += 1 }
    let child = cleaned[id_start..k].trim_matches('"').to_string();
    from = k;
    while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
    if !upper[k..].starts_with("PARTITION OF") { continue }
    k += "PARTITION OF".len();
    while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
    let p_start = k;
    while k < n && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') { k += 1 }
    let parent = cleaned[p_start..k].trim_matches('"').to_string();
    pairs.push((child, parent));
  }
  if pairs.is_empty() { return }
  // For each pair, locate parent in catalog, copy its columns onto child.
  for (child_fq, parent_fq) in pairs {
    let p_bare = parent_fq.rsplit('.').next().unwrap_or(&parent_fq).to_string();
    let c_bare = child_fq.rsplit('.').next().unwrap_or(&child_fq).to_string();
    let parent_cols: Option<Vec<Column>> = cat
      .schemas
      .iter()
      .flat_map(|s| s.tables.iter())
      .find(|t| t.name.eq_ignore_ascii_case(&p_bare))
      .map(|t| t.columns.clone());
    let Some(cols) = parent_cols else { continue };
    for schema in cat.schemas.iter_mut() {
      for t in schema.tables.iter_mut() {
        if t.name.eq_ignore_ascii_case(&c_bare) && t.columns.is_empty() {
          t.columns = cols.clone();
        }
      }
    }
  }
}

fn strip_string_literals(s: &str) -> String {
  let mut out: Vec<u8> = s.as_bytes().to_vec();
  let n = out.len();
  let mut i = 0usize;
  while i < n {
    if i + 1 < n && out[i] == b'-' && out[i + 1] == b'-' {
      while i < n && out[i] != b'\n' { out[i] = b' '; i += 1 }
      continue;
    }
    if out[i] == b'\'' {
      out[i] = b' '; i += 1;
      while i < n && out[i] != b'\'' { out[i] = b' '; i += 1 }
      if i < n { out[i] = b' '; i += 1 }
      continue;
    }
    i += 1;
  }
  String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

fn scan_policies(src: &str) -> std::collections::HashMap<String, Vec<Policy>> {
  let upper = src.to_ascii_uppercase();
  let bytes = src.as_bytes();
  let mut out: std::collections::HashMap<String, Vec<Policy>> = std::collections::HashMap::new();
  for needle in ["CREATE OR REPLACE POLICY ", "CREATE POLICY "] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(needle) {
      let after = from + rel + needle.len();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
      let name_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'"') {
        k += 1;
      }
      let name = src[name_start..k].trim_matches('"').to_string();
      let stmt_end = src[after..].find(';').map(|i| after + i).unwrap_or(src.len());
      let stmt = &src[after..stmt_end];
      let stmt_upper = stmt.to_ascii_uppercase();
      let tbl = if let Some(on_at) = stmt_upper.find(" ON ") {
        let rest = &stmt[on_at + 4..];
        let lead = rest.len() - rest.trim_start().len();
        let raw = &rest[lead..];
        let id_end = raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
        raw[..id_end].rsplit('.').next().unwrap_or(&raw[..id_end]).trim_matches('"').to_ascii_lowercase()
      } else {
        from = after;
        continue;
      };
      let permissive = if stmt_upper.contains("RESTRICTIVE") { "RESTRICTIVE" } else { "PERMISSIVE" };
      let command = ["ALL", "SELECT", "INSERT", "UPDATE", "DELETE"]
        .iter()
        .find(|c| stmt_upper.contains(&format!(" FOR {c}")))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "ALL".into());
      let roles = if let Some(to_at) = stmt_upper.find(" TO ") {
        let rest = &stmt[to_at + 4..];
        let end = rest.find(|c: char| c == ';' || c == '\n').unwrap_or(rest.len());
        let raw = rest[..end].trim();
        // Trim trailing clauses like USING / WITH CHECK.
        let raw_upper = raw.to_ascii_uppercase();
        let cut = ["USING", "WITH CHECK"]
          .iter()
          .filter_map(|kw| raw_upper.find(kw))
          .min()
          .unwrap_or(raw.len());
        raw[..cut].trim().to_string()
      } else {
        "PUBLIC".into()
      };
      let using_expr = extract_paren_after(stmt, "USING");
      let check_expr = extract_paren_after(stmt, "WITH CHECK");
      out.entry(tbl).or_default().push(Policy {
        name,
        permissive: permissive.to_string(),
        roles,
        command,
        using_expr,
        check_expr,
      });
      from = after;
    }
  }
  out
}

fn extract_paren_after(stmt: &str, kw: &str) -> Option<String> {
  let upper = stmt.to_ascii_uppercase();
  let at = upper.find(kw)?;
  let after = at + kw.len();
  let rest_bytes = stmt.as_bytes();
  let mut k = after;
  while k < rest_bytes.len() && rest_bytes[k].is_ascii_whitespace() { k += 1; }
  if k >= rest_bytes.len() || rest_bytes[k] != b'(' { return None; }
  let close = match_paren(rest_bytes, k);
  if close >= rest_bytes.len() { return None; }
  Some(stmt[k + 1..close].trim().to_string())
}

/// `COMMENT ON TABLE <tbl> IS '<text>'` -> map<table_lower, text>.
fn scan_table_comments(src: &str) -> std::collections::HashMap<String, String> {
  let upper = src.to_ascii_uppercase();
  let mut out = std::collections::HashMap::new();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find("COMMENT ON TABLE ") {
    let after = from + rel + "COMMENT ON TABLE ".len();
    let bytes = src.as_bytes();
    let mut k = after;
    while k < bytes.len() && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    let id_start = k;
    while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') {
      k += 1;
    }
    let id = &src[id_start..k];
    let bare = id.rsplit('.').next().unwrap_or(id).trim_matches('"').to_ascii_lowercase();
    while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
    if k + 2 > bytes.len() || !upper[k..].starts_with("IS") {
      from = after;
      continue;
    }
    k += 2;
    while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
    if k >= bytes.len() || bytes[k] != b'\'' {
      from = after;
      continue;
    }
    let s_start = k + 1;
    let mut s_end = s_start;
    while s_end < bytes.len() && bytes[s_end] != b'\'' { s_end += 1; }
    if s_end >= bytes.len() { break; }
    out.insert(bare, src[s_start..s_end].to_string());
    from = s_end + 1;
  }
  out
}

/// `CREATE [UNIQUE] INDEX [IF NOT EXISTS] <name> ON <tbl> (cols) [USING ...]`
/// -> map<table_lower, Vec<IndexDef>>.
fn scan_indexes(src: &str) -> std::collections::HashMap<String, Vec<IndexDef>> {
  let upper = src.to_ascii_uppercase();
  let bytes = src.as_bytes();
  let mut out: std::collections::HashMap<String, Vec<IndexDef>> = std::collections::HashMap::new();
  for (needle, unique) in [
    ("CREATE UNIQUE INDEX IF NOT EXISTS ", true),
    ("CREATE UNIQUE INDEX ", true),
    ("CREATE INDEX IF NOT EXISTS ", false),
    ("CREATE INDEX ", false),
  ] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(needle) {
      let after = from + rel + needle.len();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
      let name_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'"') {
        k += 1;
      }
      let name = src[name_start..k].trim_matches('"').to_string();
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
      if upper[k..].starts_with("ON ") {
        k += 3;
      } else {
        from = after;
        continue;
      }
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
      let tbl_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') {
        k += 1;
      }
      let raw_tbl = &src[tbl_start..k];
      let bare = raw_tbl.rsplit('.').next().unwrap_or(raw_tbl).trim_matches('"').to_ascii_lowercase();
      // Find `(`.
      while k < bytes.len() && bytes[k] != b'(' && bytes[k] != b';' { k += 1; }
      let cols = if k < bytes.len() && bytes[k] == b'(' {
        let end = match_paren(bytes, k);
        let body = &src[k + 1..end];
        split_top_commas(body)
          .into_iter()
          .map(|c| c.trim().trim_matches('"').to_string())
          .filter(|c| !c.is_empty())
          .collect()
      } else {
        Vec::new()
      };
      // Find the end of this statement.
      let stmt_end = src[name_start..].find(';').map(|i| name_start + i + 1).unwrap_or(src.len());
      let definition = src[from + rel..stmt_end].trim().to_string();
      out.entry(bare).or_default().push(IndexDef {
        name,
        columns: cols,
        unique,
        definition: Some(definition),
      });
      from = after;
    }
  }
  out
}

/// `CREATE [OR REPLACE] TRIGGER <name> {BEFORE|AFTER|INSTEAD OF}
/// {INSERT|UPDATE|DELETE|TRUNCATE} ON <tbl> FOR EACH {ROW|STATEMENT}
/// EXECUTE FUNCTION <fn>()` -> map<table_lower, Vec<Trigger>>.
fn scan_triggers(src: &str) -> std::collections::HashMap<String, Vec<Trigger>> {
  let upper = src.to_ascii_uppercase();
  let bytes = src.as_bytes();
  let mut out: std::collections::HashMap<String, Vec<Trigger>> = std::collections::HashMap::new();
  for needle in ["CREATE OR REPLACE TRIGGER ", "CREATE TRIGGER ", "CREATE CONSTRAINT TRIGGER "] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(needle) {
      let after = from + rel + needle.len();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
      let name_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'"') { k += 1; }
      let name = src[name_start..k].trim_matches('"').to_string();
      let stmt_end = src[after..].find(';').map(|i| after + i).unwrap_or(src.len());
      let stmt = &src[after..stmt_end];
      let stmt_upper = stmt.to_ascii_uppercase();
      let timing = if stmt_upper.contains("INSTEAD OF") { "INSTEAD OF" }
        else if stmt_upper.contains("BEFORE") { "BEFORE" }
        else if stmt_upper.contains("AFTER") { "AFTER" }
        else { "" };
      let mut events = Vec::new();
      if stmt_upper.contains("INSERT") { events.push("INSERT"); }
      if stmt_upper.contains("UPDATE") { events.push("UPDATE"); }
      if stmt_upper.contains("DELETE") { events.push("DELETE"); }
      if stmt_upper.contains("TRUNCATE") { events.push("TRUNCATE"); }
      let event = events.join(" OR ");
      // `ON <tbl>`.
      let tbl = if let Some(on_at) = stmt_upper.find(" ON ") {
        let after_on = on_at + 4;
        let rest = &stmt[after_on..];
        let lead = rest.len() - rest.trim_start().len();
        let raw = &rest[lead..];
        let id_end = raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
        raw[..id_end].rsplit('.').next().unwrap_or(&raw[..id_end]).trim_matches('"').to_ascii_lowercase()
      } else {
        String::new()
      };
      let granularity = if stmt_upper.contains("FOR EACH ROW") { "ROW" }
        else if stmt_upper.contains("FOR EACH STATEMENT") { "STATEMENT" }
        else { "ROW" };
      let function = if let Some(at) = stmt_upper.find("EXECUTE FUNCTION ").or_else(|| stmt_upper.find("EXECUTE PROCEDURE ")) {
        let after_kw = at + "EXECUTE FUNCTION ".len();
        let rest = &stmt[after_kw..];
        let lead = rest.len() - rest.trim_start().len();
        let raw = &rest[lead..];
        let id_end = raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.').unwrap_or(raw.len());
        raw[..id_end].to_string()
      } else {
        String::new()
      };
      if !tbl.is_empty() {
        out.entry(tbl).or_default().push(Trigger {
          name,
          timing: timing.to_string(),
          event,
          granularity: granularity.to_string(),
          function,
        });
      }
      from = after;
    }
  }
  out
}

/// Harvest `COMMENT ON COLUMN <tbl>.<col> IS '<text>'` statements
/// (case-insensitive) from raw source. Returns a `tbl.col -> text` map
/// (both keys lowercased) so attach_column_comments can match without
/// a second pass. Permissive scan -- malformed/half-written input is
/// silently skipped.
fn scan_column_comments(src: &str) -> std::collections::HashMap<String, String> {
  let upper = src.to_ascii_uppercase();
  let mut out = std::collections::HashMap::new();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find("COMMENT ON COLUMN ") {
    let after = from + rel + "COMMENT ON COLUMN ".len();
    let bytes = src.as_bytes();
    let mut k = after;
    while k < bytes.len() && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    let id_start = k;
    while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') {
      k += 1;
    }
    let id = &src[id_start..k];
    // Need at least `table.column` (two segments).
    let parts: Vec<&str> = id.split('.').map(|s| s.trim_matches('"')).collect();
    if parts.len() < 2 {
      from = after;
      continue;
    }
    let tbl = parts[parts.len() - 2].to_ascii_lowercase();
    let col = parts[parts.len() - 1].to_ascii_lowercase();
    // Look for `IS '<text>'`.
    while k < bytes.len() && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k + 2 > bytes.len() || !upper[k..].starts_with("IS") {
      from = after;
      continue;
    }
    k += 2;
    while k < bytes.len() && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= bytes.len() || bytes[k] != b'\'' {
      from = after;
      continue;
    }
    let s_start = k + 1;
    let mut s_end = s_start;
    while s_end < bytes.len() && bytes[s_end] != b'\'' {
      s_end += 1;
    }
    if s_end >= bytes.len() {
      break;
    }
    let text = src[s_start..s_end].to_string();
    out.insert(format!("{tbl}.{col}"), text);
    from = s_end + 1;
  }
  out
}

/// Locate the CREATE TABLE body for `name` and pull out every
/// PK/UNIQUE/FK constraint we can spot. Both inline-on-column and
/// table-level forms. Forgiving -- text-scan, not a real parser.
/// Walks the body of `CREATE TABLE <name> (...)` and pulls the
/// `GENERATED ALWAYS AS (expr) STORED` clause for each column that
/// has one. Returns a map of lowercase column name -> the expr
/// text (without the keywords).
fn scan_generated_for(src: &str, name: &str) -> std::collections::HashMap<String, String> {
  let upper = src.to_ascii_uppercase();
  let mut out: std::collections::HashMap<String, String> = Default::default();
  for needle in ["CREATE TABLE IF NOT EXISTS ", "CREATE TEMP TABLE ", "CREATE TEMPORARY TABLE ", "CREATE TABLE "] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(needle) {
      let after = from + rel + needle.len();
      let bytes = src.as_bytes();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1 }
      let id_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') { k += 1 }
      let raw = &src[id_start..k];
      let bare = raw.rsplit('.').next().unwrap_or(raw).trim_matches('"');
      if !bare.eq_ignore_ascii_case(name) { from = after; continue }
      while k < bytes.len() && bytes[k] != b'(' { k += 1 }
      if k >= bytes.len() { return out }
      let body_start = k + 1;
      let body_end = match_paren(bytes, k);
      let body = &src[body_start..body_end];
      for line in split_top_level(body) {
        let upper_line = line.to_ascii_uppercase();
        let Some(g_at) = upper_line.find("GENERATED") else { continue };
        let post = &line[g_at..];
        let post_upper = post.to_ascii_uppercase();
        if !post_upper.starts_with("GENERATED ALWAYS AS") { continue }
        let after_kw = g_at + "GENERATED ALWAYS AS".len();
        let rest = line[after_kw..].trim_start();
        if !rest.starts_with('(') { continue }
        let abs_open = after_kw + (line[after_kw..].len() - rest.len());
        let Some(close_rel) = match_paren_offset(&line, abs_open) else { continue };
        let expr = &line[abs_open + 1..close_rel];
        let col_name = line.split_whitespace().next().unwrap_or("").trim_matches('"').to_ascii_lowercase();
        if !col_name.is_empty() {
          out.insert(col_name, expr.to_string());
        }
      }
      return out;
    }
  }
  out
}

/// `-- @json-keys: a, b, c` annotation comment placed on its own line
/// above a jsonb column. Walks the CREATE TABLE body line-by-line,
/// remembers the most recent annotation, and attaches its keys to the
/// next non-comment, non-constraint column line. Returns
/// lowercase-column-name -> Vec<String>.
fn scan_json_keys_for(src: &str, name: &str) -> std::collections::HashMap<String, Vec<String>> {
  let upper = src.to_ascii_uppercase();
  let mut out: std::collections::HashMap<String, Vec<String>> = Default::default();
  for needle in ["CREATE TABLE IF NOT EXISTS ", "CREATE TEMP TABLE ", "CREATE TEMPORARY TABLE ", "CREATE TABLE "] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(needle) {
      let after = from + rel + needle.len();
      let bytes = src.as_bytes();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1 }
      let id_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') { k += 1 }
      let raw = &src[id_start..k];
      let bare = raw.rsplit('.').next().unwrap_or(raw).trim_matches('"');
      if !bare.eq_ignore_ascii_case(name) { from = after; continue }
      while k < bytes.len() && bytes[k] != b'(' { k += 1 }
      if k >= bytes.len() { return out }
      let body_start = k + 1;
      let body_end = match_paren(bytes, k);
      let body = &src[body_start..body_end];
      let mut pending: Option<Vec<String>> = None;
      let mut paren_depth = 0i32;
      for line in body.lines() {
        let trimmed = line.trim();
        if let Some(at) = trimmed.find("@json-keys:") {
          let payload = &trimmed[at + "@json-keys:".len()..];
          let keys: Vec<String> = payload
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect();
          if !keys.is_empty() { pending = Some(keys); }
          continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("--") { continue }
        let entered_at = paren_depth;
        for b in trimmed.as_bytes() {
          match b { b'(' => paren_depth += 1, b')' => paren_depth -= 1, _ => {} }
        }
        if entered_at != 0 { continue }
        let Some(keys) = pending.take() else { continue };
        let head = trimmed.split_whitespace().next().unwrap_or("");
        let head_upper = head.to_ascii_uppercase();
        if matches!(head_upper.as_str(), "CONSTRAINT" | "PRIMARY" | "UNIQUE" | "FOREIGN" | "CHECK" | "EXCLUDE" | "LIKE") { continue }
        let col_name = head.trim_matches('"').trim_end_matches(',').to_ascii_lowercase();
        if !col_name.is_empty() { out.insert(col_name, keys); }
      }
      return out;
    }
  }
  out
}

fn split_top_level(text: &str) -> Vec<String> {
  let mut out = Vec::new();
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(text[start..i].trim().to_string());
        start = i + 1;
      }
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      }
      _ => {}
    }
    i += 1;
  }
  out.push(text[start..].trim().to_string());
  out
}

fn match_paren_offset(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => { depth -= 1; if depth == 0 { return Some(i); } }
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      }
      _ => {}
    }
    i += 1;
  }
  None
}

fn scan_constraints_for(src: &str, name: &str) -> Vec<Constraint> {
  let upper = src.to_ascii_uppercase();
  let mut out: Vec<Constraint> = Vec::new();
  for needle in ["CREATE TABLE IF NOT EXISTS ", "CREATE TEMP TABLE ", "CREATE TEMPORARY TABLE ", "CREATE TABLE "] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(needle) {
      let after = from + rel + needle.len();
      let bytes = src.as_bytes();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      let id_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') {
        k += 1;
      }
      let raw = &src[id_start..k];
      let bare = raw.rsplit('.').next().unwrap_or(raw).trim_matches('"');
      if !bare.eq_ignore_ascii_case(name) {
        from = after;
        continue;
      }
      while k < bytes.len() && bytes[k] != b'(' {
        k += 1;
      }
      if k >= bytes.len() {
        return out;
      }
      let body_start = k + 1;
      let body_end = match_paren(bytes, k);
      let body = &src[body_start..body_end];
      out.extend(parse_constraints(body));
      return out;
    }
  }
  out
}

fn match_paren(bytes: &[u8], open: usize) -> usize {
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = open;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return i;
        }
      }
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      }
      _ => {}
    }
    i += 1;
  }
  n
}

fn parse_constraints(body: &str) -> Vec<Constraint> {
  let mut out = Vec::new();
  for entry in split_top_commas(body) {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
      continue;
    }
    let upper = trimmed.to_ascii_uppercase();
    // Table-level: PRIMARY KEY (...), UNIQUE (...), FOREIGN KEY (...) REFERENCES ...
    if upper.starts_with("PRIMARY KEY") {
      if let Some(cols) = paren_csv(trimmed) {
        out.push(Constraint {
          name: "pk".into(),
          kind: ConstraintKind::PrimaryKey,
          columns: cols,
          references: None,
          definition: Some(trimmed.to_string()),
        });
      }
      continue;
    }
    if upper.starts_with("UNIQUE") {
      if let Some(cols) = paren_csv(trimmed) {
        out.push(Constraint {
          name: "uniq".into(),
          kind: ConstraintKind::Unique,
          columns: cols,
          references: None,
          definition: Some(trimmed.to_string()),
        });
      }
      continue;
    }
    if upper.starts_with("FOREIGN KEY") {
      if let (Some(local), Some(refs)) = (paren_csv(trimmed), parse_references(trimmed)) {
        out.push(Constraint {
          name: "fk".into(),
          kind: ConstraintKind::ForeignKey,
          columns: local,
          references: Some(refs),
          definition: Some(trimmed.to_string()),
        });
      }
      continue;
    }
    if upper.starts_with("CHECK") {
      out.push(Constraint {
        name: "ck".into(),
        kind: ConstraintKind::Check,
        columns: Vec::new(),
        references: None,
        definition: Some(trimmed.to_string()),
      });
      continue;
    }
    if upper.starts_with("CONSTRAINT") {
      // CONSTRAINT <name> {PK|UNIQUE|FK|CHECK} ...
      let rest = trimmed[10..].trim_start();
      let name_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
      let cname = rest[..name_end].trim_matches('"').to_string();
      let body = rest[name_end..].trim_start();
      let body_upper = body.to_ascii_uppercase();
      if body_upper.starts_with("PRIMARY KEY") {
        out.push(Constraint {
          name: cname,
          kind: ConstraintKind::PrimaryKey,
          columns: paren_csv(body).unwrap_or_default(),
          references: None,
          definition: Some(body.to_string()),
        });
      } else if body_upper.starts_with("UNIQUE") {
        out.push(Constraint {
          name: cname,
          kind: ConstraintKind::Unique,
          columns: paren_csv(body).unwrap_or_default(),
          references: None,
          definition: Some(body.to_string()),
        });
      } else if body_upper.starts_with("FOREIGN KEY") {
        out.push(Constraint {
          name: cname,
          kind: ConstraintKind::ForeignKey,
          columns: paren_csv(body).unwrap_or_default(),
          references: parse_references(body),
          definition: Some(body.to_string()),
        });
      } else if body_upper.starts_with("CHECK") {
        out.push(Constraint {
          name: cname,
          kind: ConstraintKind::Check,
          columns: Vec::new(),
          references: None,
          definition: Some(body.to_string()),
        });
      }
      continue;
    }
    // Inline column form: `<col> <type> ... [PRIMARY KEY] [REFERENCES ...]`
    let col = trimmed.split_whitespace().next().unwrap_or("").trim_matches('"').to_string();
    if col.is_empty() {
      continue;
    }
    if upper.contains(" PRIMARY KEY") || upper.contains("\tPRIMARY KEY") {
      out.push(Constraint {
        name: format!("pk_{col}"),
        kind: ConstraintKind::PrimaryKey,
        columns: vec![col.clone()],
        references: None,
        definition: Some(trimmed.to_string()),
      });
    }
    if upper.contains("UNIQUE") && !upper.starts_with("UNIQUE") {
      out.push(Constraint {
        name: format!("uniq_{col}"),
        kind: ConstraintKind::Unique,
        columns: vec![col.clone()],
        references: None,
        definition: Some(trimmed.to_string()),
      });
    }
    if let Some(refs) = inline_references(trimmed) {
      out.push(Constraint {
        name: format!("fk_{col}"),
        kind: ConstraintKind::ForeignKey,
        columns: vec![col],
        references: Some(refs),
        definition: Some(trimmed.to_string()),
      });
    }
  }
  out
}

/// `(col1, col2)` -> `["col1", "col2"]`. None if no first `(`.
fn paren_csv(s: &str) -> Option<Vec<String>> {
  let open = s.find('(')?;
  let close = match_paren(s.as_bytes(), open);
  if close >= s.len() {
    return None;
  }
  Some(
    s[open + 1..close]
      .split(',')
      .map(|c| c.trim().trim_matches('"').to_string())
      .filter(|c| !c.is_empty())
      .collect(),
  )
}

/// `REFERENCES <tbl>(<col>)` or `REFERENCES <schema>.<tbl>(<col>)`.
fn parse_references(s: &str) -> Option<ConstraintRef> {
  let upper = s.to_ascii_uppercase();
  let at = upper.find("REFERENCES")?;
  let rest = s[at + "REFERENCES".len()..].trim_start();
  let name_end = rest
    .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"')
    .unwrap_or(rest.len());
  let raw = rest[..name_end].trim_matches('"');
  let (schema, table) = if let Some(dot) = raw.find('.') {
    (raw[..dot].trim_matches('"').to_string(), raw[dot + 1..].trim_matches('"').to_string())
  } else {
    ("public".into(), raw.to_string())
  };
  let cols = paren_csv(&rest[name_end..]).unwrap_or_default();
  Some(ConstraintRef { schema, table, columns: cols })
}

fn inline_references(s: &str) -> Option<ConstraintRef> {
  if !s.to_ascii_uppercase().contains("REFERENCES") {
    return None;
  }
  parse_references(s)
}


/// The conventional roles that exist in any fresh Postgres install.
/// Returned as a Vec so callers can union them into the offline cat.
pub fn default_offline_roles() -> Vec<String> {
  vec![
    "postgres".into(),
    "pg_read_all_data".into(),
    "pg_write_all_data".into(),
    "pg_monitor".into(),
    "pg_signal_backend".into(),
  ]
}

fn scan_sequences(src: &str) -> Vec<Sequence> {
  scan_create_named(src, &["CREATE SEQUENCE "])
    .into_iter()
    .map(|name| Sequence {
      schema: "public".into(),
      name,
      data_type: "bigint".into(),
      start_value: 1,
      min_value: 1,
      max_value: i64::MAX,
      increment_by: 1,
      cycle: false,
      owned_by_column: None,
      comment: Some("defined in current buffer".into()),
    })
    .collect()
}

fn scan_extensions(src: &str) -> Vec<Extension> {
  let mut out = Vec::new();
  for name in scan_create_named(src, &["CREATE EXTENSION IF NOT EXISTS ", "CREATE EXTENSION "]) {
    out.push(Extension {
      name: name.trim_matches('"').to_string(),
      schema: "public".into(),
      version: "?".into(),
      comment: Some("declared in current buffer".into()),
    });
  }
  out
}

fn scan_types(src: &str) -> Vec<Type> {
  let mut out = Vec::new();
  let upper = src.to_ascii_uppercase();
  for (prefix, kind) in [
    ("CREATE TYPE ", TypeKind::Composite),
    ("CREATE DOMAIN ", TypeKind::Domain),
  ] {
    for name in scan_create_named_with_upper(src, &upper, prefix) {
      // Peek a few chars past the name for AS ENUM -> Enum.
      let resolved_kind = if name_followed_by_as_enum(src, &upper, prefix, &name) {
        TypeKind::Enum
      } else {
        kind
      };
      out.push(Type { schema: "public".into(), name, kind: resolved_kind });
    }
  }
  out
}

fn name_followed_by_as_enum(src: &str, upper: &str, prefix: &str, name: &str) -> bool {
  let needle = format!("{prefix}{name}");
  let needle_upper = needle.to_ascii_uppercase();
  let Some(rel) = upper.find(&needle_upper) else { return false; };
  let after = rel + needle.len();
  let tail = src[after..].trim_start();
  tail.to_ascii_uppercase().starts_with("AS ENUM")
}

/// Pull the type out of `RETURNS <type>` in a CREATE FUNCTION DDL.
/// Stops at `AS`, `LANGUAGE`, `$$`, or newline. Returns "?" when
/// the clause isn't found.
fn extract_returns(ddl: &str) -> String {
  let upper = ddl.to_ascii_uppercase();
  let Some(at) = upper.find(" RETURNS ") else { return "?".into() };
  let after = at + 9;
  let rest = &ddl[after..];
  let stop = rest
    .find(|c: char| c == '\n')
    .or_else(|| {
      let u = rest.to_ascii_uppercase();
      u.find(" AS ").or_else(|| u.find(" LANGUAGE "))
    })
    .unwrap_or(rest.len());
  let raw = rest[..stop].trim();
  // Drop trailing punctuation / TABLE() schema noise.
  let bare = raw.split_whitespace().next().unwrap_or(raw).trim_end_matches(',');
  if bare.is_empty() { "?".into() } else { bare.to_string() }
}

fn scan_functions(src: &str) -> Vec<Function> {
  let upper = src.to_ascii_uppercase();
  let bytes = src.as_bytes();
  let mut out = Vec::new();
  for prefix in [
    "CREATE OR REPLACE FUNCTION ",
    "CREATE FUNCTION ",
    "CREATE PROCEDURE ",
    "CREATE OR REPLACE PROCEDURE ",
    "CREATE AGGREGATE ",
    "CREATE OR REPLACE AGGREGATE ",
  ] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(prefix) {
      let stmt_start = from + rel;
      let after = stmt_start + prefix.len();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
      let name_start = k;
      while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.') {
        k += 1;
      }
      if k == name_start {
        from = after;
        continue;
      }
      let raw = &src[name_start..k];
      let bare = raw.rsplit('.').next().unwrap_or(raw).to_string();
      // Find the full DDL up to the matching `;` after the dollar-
      // quoted body (or just up to the end of file if unterminated).
      let stmt_end = find_function_end(src, k);
      let full_ddl = src[stmt_start..stmt_end].trim().to_string();
      // Extract RETURNS <type> from the DDL header (before AS $$).
      let return_type = extract_returns(&full_ddl);
      out.push(Function {
        schema: "public".into(),
        name: bare,
        arguments: Vec::<FunctionArg>::new(),
        return_type,
        // Render layer reads `comment` for the source block when it
        // starts with CREATE; ship the full DDL so the hover shows
        // the body inline.
        comment: Some(full_ddl),
      });
      from = after;
    }
  }
  out
}

/// Walk from `pos` to the end of the CREATE FUNCTION statement.
/// Respects the dollar-quoted body so `;` inside `$$ ... $$` doesn't
/// terminate the scan prematurely.
fn find_function_end(src: &str, start: usize) -> usize {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut i = start;
  let mut in_dollar: Option<String> = None;
  while i < n {
    if let Some(tag) = &in_dollar {
      if i + tag.len() <= n && &src[i..i + tag.len()] == tag.as_str() {
        i += tag.len();
        in_dollar = None;
        continue;
      }
      i += 1;
      continue;
    }
    let c = bytes[i];
    if c == b'$' {
      let mut j = i + 1;
      while j < n && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
      }
      if j < n && bytes[j] == b'$' {
        in_dollar = Some(src[i..=j].to_string());
        i = j + 1;
        continue;
      }
    }
    if c == b';' {
      return i + 1;
    }
    if c == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' { i += 1; }
      i = (i + 1).min(n);
      continue;
    }
    i += 1;
  }
  n
}

fn scan_roles(src: &str) -> Vec<String> {
  let mut out: std::collections::BTreeSet<String> = default_offline_roles().into_iter().collect();
  for prefix in ["CREATE ROLE ", "CREATE USER ", "CREATE GROUP "] {
    for name in scan_create_named(src, &[prefix]) {
      out.insert(name);
    }
  }
  out.into_iter().collect()
}

/// Walk the source for each prefix; for each match return the identifier
/// (possibly schema-qualified, returned bare) immediately following.
fn scan_create_named(src: &str, prefixes: &[&str]) -> Vec<String> {
  let upper = src.to_ascii_uppercase();
  let mut out = Vec::new();
  for prefix in prefixes {
    out.extend(scan_create_named_with_upper(src, &upper, prefix));
  }
  out.sort();
  out.dedup();
  out
}

fn scan_create_named_with_upper(src: &str, upper: &str, prefix: &str) -> Vec<String> {
  let bytes = src.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find(prefix) {
    let after = from + rel + prefix.len();
    let mut k = after;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    let id_start = k;
    while k < n && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') {
      k += 1;
    }
    if k > id_start {
      let raw = &src[id_start..k];
      let bare = raw.rsplit('.').next().unwrap_or(raw).trim_matches('"').to_string();
      if !bare.is_empty() {
        out.push(bare);
      }
    }
    from = after;
  }
  out
}

/// Text-scan fallback: harvest the column-name list from a possibly
/// unclosed `CREATE TABLE <name> (` body. Returns names in order of
/// appearance. Used when the SQL parser can't produce a clean AST yet
/// (cursor inside the body the user is still typing).
pub fn buffer_column_names(source: &str, table: &str) -> Vec<String> {
  let upper_target = table.to_ascii_uppercase();
  let mut from = 0usize;
  let upper = source.to_ascii_uppercase();
  while let Some(rel) = upper[from..].find("CREATE TABLE") {
    let start = from + rel;
    let after = start + "CREATE TABLE".len();
    let mut rest_start = after;
    // Skip whitespace + optional IF NOT EXISTS.
    let after_str = &source[rest_start..];
    let trim_lead = after_str.len() - after_str.trim_start().len();
    rest_start += trim_lead;
    if upper[rest_start..].starts_with("IF NOT EXISTS") {
      rest_start += "IF NOT EXISTS".len();
      let after2 = &source[rest_start..];
      rest_start += after2.len() - after2.trim_start().len();
    }
    // Read the name token.
    let name: String =
      source[rest_start..].chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
    let name_only = name.rsplit('.').next().unwrap_or(&name);
    if !name_only.eq_ignore_ascii_case(table) && !name.to_ascii_uppercase().contains(&upper_target) {
      from = after;
      continue;
    }
    // Find the body opener.
    let body_open = source[rest_start + name.len()..].find('(');
    let Some(rel_open) = body_open else {
      from = after;
      continue;
    };
    let open = rest_start + name.len() + rel_open + 1;
    // Walk paren depth to find close (or EOF when unclosed).
    let bytes = source.as_bytes();
    let n = bytes.len();
    let mut depth = 1i32;
    let mut i = open;
    let close;
    loop {
      if i >= n {
        close = n;
        break;
      }
      match bytes[i] {
        b'\'' => {
          i += 1;
          while i < n {
            if bytes[i] == b'\'' {
              i += 1;
              break;
            }
            i += 1;
          }
        },
        b'(' => {
          depth += 1;
          i += 1;
        },
        b')' => {
          depth -= 1;
          i += 1;
          if depth == 0 {
            close = i - 1;
            break;
          }
        },
        _ => i += 1,
      }
    }
    let body = &source[open..close.min(n)];
    return harvest_names(body);
  }
  Vec::new()
}

fn harvest_names(body: &str) -> Vec<String> {
  let mut out = Vec::new();
  for raw in split_top_commas(body) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
      continue;
    }
    let head_upper = trimmed.to_ascii_uppercase();
    // Skip table-level constraint lines.
    let first = head_upper.split_ascii_whitespace().next().unwrap_or("");
    if matches!(first, "CONSTRAINT" | "PRIMARY" | "FOREIGN" | "UNIQUE" | "CHECK" | "EXCLUDE" | "LIKE") {
      continue;
    }
    // First identifier is the column name. Skip the trailing partial
    // token that the cursor is in the middle of -- the user already
    // sees that as they type.
    let name: String = trimmed.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    if !name.is_empty() && !out.contains(&name) {
      out.push(name);
    }
  }
  out
}

fn split_top_commas(s: &str) -> Vec<&str> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut depth = 0i32;
  let mut last = 0usize;
  let mut out = Vec::new();
  let mut i = 0usize;
  while i < n {
    match bytes[i] {
      b'\'' => {
        i += 1;
        while i < n {
          if bytes[i] == b'\'' {
            i += 1;
            break;
          }
          i += 1;
        }
      },
      b'(' => {
        depth += 1;
        i += 1;
      },
      b')' => {
        depth -= 1;
        i += 1;
      },
      b',' if depth == 0 => {
        out.push(&s[last..i]);
        last = i + 1;
        i += 1;
      },
      _ => i += 1,
    }
  }
  out.push(&s[last..]);
  out
}

/// Merge two catalogs into one. `live` wins on name collisions across
/// every collection (tables, sequences, types, extensions, functions,
/// roles). Derived entries fill in only where live has a gap, so a
/// live DB connection always takes precedence over text-scanned
/// guesses but offline use still gets the full set.
pub fn merge(live: &Catalog, derived: &Catalog) -> Catalog {
  let mut out = live.clone();
  for ds in &derived.schemas {
    let target = match out.schemas.iter_mut().find(|s| s.name == ds.name) {
      Some(s) => s,
      None => {
        out.schemas.push(Schema { name: ds.name.clone(), tables: Vec::new() });
        out.schemas.last_mut().unwrap()
      },
    };
    for dt in &ds.tables {
      if !target.tables.iter().any(|t| t.name == dt.name) {
        target.tables.push(dt.clone());
      }
    }
  }
  for s in &derived.sequences {
    if !out.sequences.iter().any(|x| x.name.eq_ignore_ascii_case(&s.name)) {
      out.sequences.push(s.clone());
    }
  }
  for t in &derived.types {
    if !out.types.iter().any(|x| x.name.eq_ignore_ascii_case(&t.name)) {
      out.types.push(t.clone());
    }
  }
  for e in &derived.extensions {
    if !out.extensions.iter().any(|x| x.name.eq_ignore_ascii_case(&e.name)) {
      out.extensions.push(e.clone());
    }
  }
  for f in &derived.functions {
    if !out.functions.iter().any(|x| x.name.eq_ignore_ascii_case(&f.name)) {
      out.functions.push(f.clone());
    }
  }
  for r in &derived.roles {
    if !out.roles.iter().any(|x| x.eq_ignore_ascii_case(r)) {
      out.roles.push(r.clone());
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use dsl_parse::Dialect;

  #[test]
  fn json_keys_annotation_populates_column() {
    let src = "CREATE TABLE event (\n  id int,\n  -- @json-keys: name, age, tier\n  payload jsonb\n);\n";
    let parsed = dsl_parse::parse(src, Dialect::Postgres);
    let cat = from_source(&parsed, src);
    let tbl = cat
      .schemas
      .iter()
      .flat_map(|s| s.tables.iter())
      .find(|t| t.name == "event")
      .expect("event table");
    let payload = tbl.columns.iter().find(|c| c.name == "payload").expect("payload col");
    assert_eq!(payload.json_keys.as_deref(), Some(&["name".to_string(), "age".to_string(), "tier".to_string()][..]));
    let id = tbl.columns.iter().find(|c| c.name == "id").expect("id col");
    assert!(id.json_keys.is_none());
  }

  #[test]
  fn json_keys_annotation_quoted_keys() {
    let src = "CREATE TABLE t (\n  -- @json-keys: \"a-b\", c\n  data jsonb\n);\n";
    let parsed = dsl_parse::parse(src, Dialect::Postgres);
    let cat = from_source(&parsed, src);
    let tbl = cat.schemas.iter().flat_map(|s| s.tables.iter()).find(|t| t.name == "t").unwrap();
    let col = tbl.columns.iter().find(|c| c.name == "data").unwrap();
    assert_eq!(col.json_keys.as_deref(), Some(&["a-b".to_string(), "c".to_string()][..]));
  }
}
