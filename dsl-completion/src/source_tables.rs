//! Build a synthetic catalog from CREATE TABLE statements in the current
//! file, so completion / hover surface tables the user is actively
//! defining even before the DB knows about them.
//!
//! This is purely additive: the live catalog from `dsl-catalog` and the
//! source-derived one are merged at lookup time. When both define the
//! same table, the live catalog wins because it has richer metadata
//! (constraints, indexes, comments).

use dsl_catalog::{Catalog, Column, Schema, Table, TableKind, CATALOG_VERSION};
use dsl_parse::{ParsedFile, StatementKind};

/// Build a [`Catalog`] from every CREATE TABLE statement in `file`.
/// Returns an empty catalog if none are found.
pub fn from_file(file: &ParsedFile) -> Catalog {
    let mut public = Schema { name: "public".into(), tables: Vec::new() };
    let mut other: std::collections::BTreeMap<String, Schema> = Default::default();

    for stmt in &file.statements {
        let StatementKind::CreateTable(ct) = &stmt.kind else { continue; };
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
    if !public.tables.is_empty() { schemas.push(public); }
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
        let name: String = source[rest_start..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
            .collect();
        let name_only = name.rsplit('.').next().unwrap_or(&name);
        if !name_only.eq_ignore_ascii_case(table) && !name.to_ascii_uppercase().contains(&upper_target) {
            from = after;
            continue;
        }
        // Find the body opener.
        let body_open = source[rest_start + name.len()..].find('(');
        let Some(rel_open) = body_open else { from = after; continue; };
        let open = rest_start + name.len() + rel_open + 1;
        // Walk paren depth to find close (or EOF when unclosed).
        let bytes = source.as_bytes();
        let n = bytes.len();
        let mut depth = 1i32;
        let mut i = open;
        let close;
        loop {
            if i >= n { close = n; break; }
            match bytes[i] {
                b'\'' => {
                    i += 1;
                    while i < n {
                        if bytes[i] == b'\'' { i += 1; break; }
                        i += 1;
                    }
                }
                b'(' => { depth += 1; i += 1; }
                b')' => { depth -= 1; i += 1; if depth == 0 { close = i - 1; break; } }
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
        if trimmed.is_empty() { continue; }
        let head_upper = trimmed.to_ascii_uppercase();
        // Skip table-level constraint lines.
        let first = head_upper.split_ascii_whitespace().next().unwrap_or("");
        if matches!(first, "CONSTRAINT" | "PRIMARY" | "FOREIGN" | "UNIQUE" | "CHECK" | "EXCLUDE" | "LIKE") {
            continue;
        }
        // First identifier is the column name. Skip the trailing partial
        // token that the cursor is in the middle of -- the user already
        // sees that as they type.
        let name: String = trimmed
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
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
                    if bytes[i] == b'\'' { i += 1; break; }
                    i += 1;
                }
            }
            b'(' => { depth += 1; i += 1; }
            b')' => { depth -= 1; i += 1; }
            b',' if depth == 0 => { out.push(&s[last..i]); last = i + 1; i += 1; }
            _ => i += 1,
        }
    }
    out.push(&s[last..]);
    out
}

/// Merge two catalogs into one. `live` wins on name collisions.
pub fn merge(live: &Catalog, derived: &Catalog) -> Catalog {
    let mut out = live.clone();
    for ds in &derived.schemas {
        let target = match out.schemas.iter_mut().find(|s| s.name == ds.name) {
            Some(s) => s,
            None => {
                out.schemas.push(Schema { name: ds.name.clone(), tables: Vec::new() });
                out.schemas.last_mut().unwrap()
            }
        };
        for dt in &ds.tables {
            if !target.tables.iter().any(|t| t.name == dt.name) {
                target.tables.push(dt.clone());
            }
        }
    }
    out
}
