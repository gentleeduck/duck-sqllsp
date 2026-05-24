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
  CATALOG_VERSION, Catalog, Column, Extension, Function, FunctionArg, Schema, Sequence, Table,
  TableKind, Type, TypeKind,
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
        })
        .collect(),
      constraints: Vec::new(),
      indexes: Vec::new(),
      triggers: Vec::new(),
      policies: Vec::new(),
      comment: Some("defined in current file".into()),
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
  cat
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

fn scan_functions(src: &str) -> Vec<Function> {
  let upper = src.to_ascii_uppercase();
  let mut out = Vec::new();
  for prefix in ["CREATE OR REPLACE FUNCTION ", "CREATE FUNCTION ", "CREATE PROCEDURE "] {
    for name in scan_create_named_with_upper(src, &upper, prefix) {
      out.push(Function {
        schema: "public".into(),
        name,
        arguments: Vec::<FunctionArg>::new(),
        return_type: "?".into(),
        comment: Some("defined in current buffer".into()),
      });
    }
  }
  out
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
