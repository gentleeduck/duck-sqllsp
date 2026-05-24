//! `textDocument/definition` handler.
//!
//! Resolves every cursor target we can pin down to a buffer-local
//! definition:
//!
//!   - Table / view / sequence / type / domain name -> `CREATE <kind>`
//!   - Function / procedure name -> `CREATE FUNCTION|PROCEDURE`
//!   - Trigger name -> `CREATE TRIGGER`
//!   - Index name -> `CREATE INDEX`
//!   - Alias -> `FROM <table> [AS] <alias>` site
//!   - Column reference -> column declaration inside its CREATE TABLE
//!
//! In every case the returned range points at the **name token**, never
//! the whole statement, so the editor's cursor lands on the identifier.

use crate::handlers::position;
use crate::state::ServerState;
use ropey::Rope;
use text_size::TextRange;
use tower_lsp::lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range};

pub fn run(state: &ServerState, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
  let uri = params.text_document_position_params.text_document.uri;
  let _g = crate::handlers::perf::Guard::with_uri("definition", &uri);
  let doc = state.documents.get(&uri)?;
  if doc.too_large() {
    return None;
  }
  let offset = position::to_offset(&doc.rope, params.text_document_position_params.position);
  let text = &doc.text;

  // Cursor may sit on `a.b` (alias.col). Walk both segments.
  let (left, right) = split_dotted(text, u32::from(offset) as usize);

  // Right side wins when on a column reference -- jump to its column
  // declaration inside the qualified table's CREATE TABLE body.
  if let (Some(l), Some(r)) = (left.as_ref(), right.as_ref()) {
    if let Some(range) = column_def_in_create_table(text, l, r) {
      return Some(scalar(uri.clone(), &doc.rope, range));
    }
  }

  let token = right.clone().or_else(|| left.clone())?;

  // 1. CREATE <kind> <name> -- text scan over every common DDL keyword.
  //    Walks every open buffer so definitions split across files
  //    (migrations / seeds / function libraries) all resolve.
  const DDL_NAMES: &[&str] = &[
    "CREATE OR REPLACE FUNCTION",
    "CREATE FUNCTION",
    "CREATE OR REPLACE PROCEDURE",
    "CREATE PROCEDURE",
    "CREATE OR REPLACE TRIGGER",
    "CREATE TRIGGER",
    "CREATE OR REPLACE VIEW",
    "CREATE VIEW",
    "CREATE MATERIALIZED VIEW",
    "CREATE UNIQUE INDEX IF NOT EXISTS",
    "CREATE UNIQUE INDEX",
    "CREATE INDEX IF NOT EXISTS",
    "CREATE INDEX",
    "CREATE TABLE IF NOT EXISTS",
    "CREATE TEMPORARY TABLE",
    "CREATE TEMP TABLE",
    "CREATE TABLE",
    "CREATE SEQUENCE IF NOT EXISTS",
    "CREATE SEQUENCE",
    "CREATE TYPE",
    "CREATE DOMAIN",
    "CREATE SCHEMA IF NOT EXISTS",
    "CREATE SCHEMA",
    "CREATE EXTENSION IF NOT EXISTS",
    "CREATE EXTENSION",
    "CREATE OR REPLACE POLICY",
    "CREATE POLICY",
    "CREATE OR REPLACE AGGREGATE",
    "CREATE AGGREGATE",
    "CREATE ROLE",
    "CREATE USER",
    "CREATE GROUP",
  ];

  // Search cursor's doc first (most common hit), then every other buffer.
  let upper = text.to_ascii_uppercase();
  for needle in DDL_NAMES {
    if let Some(r) = find_def_name(&upper, text, needle, &token) {
      return Some(scalar(uri.clone(), &doc.rope, r));
    }
  }
  // PL/pgSQL local: DECLARE <name> ... in the same buffer.
  if let Some(r) = find_declare_site(text, &token, u32::from(offset) as usize) {
    return Some(scalar(uri.clone(), &doc.rope, r));
  }
  // CTE: WITH <name> AS (...) (or comma-separated subsequent CTEs).
  if let Some(r) = find_cte_site(text, &token, u32::from(offset) as usize) {
    return Some(scalar(uri.clone(), &doc.rope, r));
  }
  // Alias site in this buffer.
  if let Some(r) = find_alias_site(text, &upper, &token) {
    return Some(scalar(uri.clone(), &doc.rope, r));
  }

  // Workspace-wide DDL lookup: scan every other open buffer.
  for (other_uri, other_doc) in state.documents.snapshot() {
    if other_uri == uri { continue; }
    let other_upper = other_doc.text.to_ascii_uppercase();
    for needle in DDL_NAMES {
      if let Some(r) = find_def_name(&other_upper, &other_doc.text, needle, &token) {
        return Some(scalar(other_uri.clone(), &other_doc.rope, r));
      }
    }
  }

  // Disk fallback: walk every .sql file in the workspace root so
  // go-def resolves into files the user hasn't opened yet.
  if let Some(root) = state.workspace_root.read().clone() {
    if let Some(loc) = find_in_workspace_files(&root, &token) {
      return Some(GotoDefinitionResponse::Scalar(loc));
    }
  }

  None
}

fn find_in_workspace_files(root: &std::path::Path, name: &str) -> Option<tower_lsp::lsp_types::Location> {
  const DDL_NAMES: &[&str] = &[
    "CREATE OR REPLACE FUNCTION", "CREATE FUNCTION",
    "CREATE OR REPLACE PROCEDURE", "CREATE PROCEDURE",
    "CREATE OR REPLACE TRIGGER", "CREATE TRIGGER",
    "CREATE OR REPLACE VIEW", "CREATE VIEW",
    "CREATE MATERIALIZED VIEW",
    "CREATE UNIQUE INDEX IF NOT EXISTS", "CREATE UNIQUE INDEX",
    "CREATE INDEX IF NOT EXISTS", "CREATE INDEX",
    "CREATE TABLE IF NOT EXISTS", "CREATE TEMPORARY TABLE",
    "CREATE TEMP TABLE", "CREATE TABLE",
    "CREATE SEQUENCE IF NOT EXISTS", "CREATE SEQUENCE",
    "CREATE TYPE", "CREATE DOMAIN",
    "CREATE SCHEMA IF NOT EXISTS", "CREATE SCHEMA",
    "CREATE EXTENSION IF NOT EXISTS", "CREATE EXTENSION",
    "CREATE OR REPLACE POLICY", "CREATE POLICY",
    "CREATE OR REPLACE AGGREGATE", "CREATE AGGREGATE",
    "CREATE ROLE", "CREATE USER", "CREATE GROUP",
  ];
  let mut count = 0usize;
  let result = std::sync::Arc::new(std::sync::Mutex::new(None::<tower_lsp::lsp_types::Location>));
  let res2 = result.clone();
  walk_sql(root, 5000, &mut count, &mut |path| {
    if res2.lock().ok().and_then(|g| g.clone()).is_some() {
      return;
    }
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let upper = text.to_ascii_uppercase();
    for needle in DDL_NAMES {
      if let Some(r) = find_def_name(&upper, &text, needle, name) {
        let rope = ropey::Rope::from_str(&text);
        let Ok(uri) = tower_lsp::lsp_types::Url::from_file_path(path) else { return };
        let loc = tower_lsp::lsp_types::Location { uri, range: to_lsp_range(&rope, r) };
        if let Ok(mut g) = res2.lock() { *g = Some(loc); }
        return;
      }
    }
  });
  result.lock().ok().and_then(|g| g.clone())
}

fn walk_sql(root: &std::path::Path, cap: usize, count: &mut usize, f: &mut impl FnMut(&std::path::Path)) {
  if *count >= cap { return; }
  let Ok(rd) = std::fs::read_dir(root) else { return };
  for entry in rd.flatten() {
    if *count >= cap { return; }
    let path = entry.path();
    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
      if name.starts_with('.') || matches!(name, "node_modules" | "target" | "dist" | "build" | "vendor" | "out") {
        continue;
      }
    }
    if path.is_dir() {
      walk_sql(&path, cap, count, f);
    } else if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
      if matches!(ext.to_ascii_lowercase().as_str(), "sql" | "pgsql" | "psql") {
        *count += 1;
        f(&path);
      }
    }
  }
}

fn scalar(uri: tower_lsp::lsp_types::Url, rope: &Rope, r: TextRange) -> GotoDefinitionResponse {
  GotoDefinitionResponse::Scalar(Location { uri, range: to_lsp_range(rope, r) })
}

/// Find `<needle> <name>` in the source, returning the TextRange of the
/// name token (whitespace skipped). Quoted identifiers (`"foo bar"`) are
/// handled by treating the whole quoted run as the candidate.
fn find_def_name(upper: &str, text: &str, needle: &str, name: &str) -> Option<TextRange> {
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find(needle) {
    let after = from + rel + needle.len();
    // Must be at a word boundary so `CREATE TABLE` doesn't match
    // inside `CREATE TABLES_PARTITION` etc.
    let prev_ok = rel == 0 || !is_word_char(upper.as_bytes()[from + rel - 1] as char);
    let next_ok = upper.as_bytes().get(after).map_or(true, |b| !is_word_char(*b as char));
    if !prev_ok || !next_ok {
      from = after;
      continue;
    }
    let rest = &text[after..];
    let ws = rest.len() - rest.trim_start().len();
    let name_start = after + ws;
    // Quoted identifier.
    if text.as_bytes().get(name_start) == Some(&b'"') {
      let mut end = name_start + 1;
      while end < text.len() && text.as_bytes()[end] != b'"' {
        end += 1;
      }
      let cand = &text[name_start + 1..end];
      if cand.eq_ignore_ascii_case(name) {
        return Some(TextRange::new((name_start as u32).into(), ((end + 1) as u32).into()));
      }
      from = after;
      continue;
    }
    let bytes = text.as_bytes();
    let mut end = name_start;
    while end < bytes.len() && is_word_char(bytes[end] as char) {
      end += 1;
    }
    let candidate = &text[name_start..end];
    if candidate.eq_ignore_ascii_case(name) {
      return Some(TextRange::new((name_start as u32).into(), (end as u32).into()));
    }
    from = after;
  }
  None
}

/// Find `<alias>` declared via `FROM <table> [AS] <alias>` or
/// `JOIN <table> [AS] <alias>` and return the alias token range.
/// Locate `DECLARE <name>` for a PL/pgSQL local variable that is in
/// scope at byte position `cursor_pos`. Walks back from the cursor to
/// the nearest enclosing `$$ ... DECLARE ... BEGIN` window and searches
/// the DECLARE section for the name token.
fn find_declare_site(text: &str, name: &str, cursor_pos: usize) -> Option<TextRange> {
  let upper = text.to_ascii_uppercase();
  // Find the most recent `$$` opener before the cursor and the next
  // `BEGIN` after that opener. The DECLARE section lives between
  // those two anchors.
  let dollar = upper[..cursor_pos].rfind("$$")?;
  // Slice from dollar to cursor (or to a closing $$).
  let win_end = upper[dollar + 2..]
    .find("$$")
    .map(|i| dollar + 2 + i)
    .unwrap_or(text.len());
  let win = &text[dollar + 2..win_end];
  let win_upper = upper[dollar + 2..win_end].to_string();
  let decl_at = win_upper.find("DECLARE")?;
  let begin_at = win_upper[decl_at..].find("BEGIN").map(|i| decl_at + i).unwrap_or(win.len());
  let body = &win[decl_at + "DECLARE".len()..begin_at];
  let body_start_abs = dollar + 2 + decl_at + "DECLARE".len();

  // Each line in the DECLARE section is `name [CONSTANT] type ...;` --
  // we just want the leading identifier.
  for stmt in body.split(';') {
    let s = stmt.trim_start();
    let lead_ws = stmt.len() - s.len();
    let id_end = s.chars().take_while(|c| is_word_char(*c)).count();
    if id_end == 0 { continue; }
    let candidate = &s[..id_end];
    if candidate.eq_ignore_ascii_case(name) {
      let rel = stmt.as_ptr() as usize - body.as_ptr() as usize;
      let abs_start = body_start_abs + rel + lead_ws;
      let abs_end = abs_start + id_end;
      return Some(TextRange::new((abs_start as u32).into(), (abs_end as u32).into()));
    }
  }
  None
}

/// Locate the CTE binding `WITH <name> AS (...)` or `, <name> AS (...)`
/// whose body is the lexical predecessor of `cursor_pos`. Returns the
/// range of the CTE name token.
fn find_cte_site(text: &str, name: &str, cursor_pos: usize) -> Option<TextRange> {
  let upper = text.to_ascii_uppercase();
  // Find the most recent `WITH ` (word-boundary) before the cursor.
  let mut from = 0usize;
  let mut last_with: Option<usize> = None;
  while let Some(rel) = upper[from..cursor_pos].find("WITH ") {
    let at = from + rel;
    let prev_ok = at == 0 || !is_word_char(upper.as_bytes()[at - 1] as char);
    if prev_ok { last_with = Some(at); }
    from = at + 5;
  }
  let with_at = last_with?;
  // Scan forward through CTE bindings: `name AS (...)` separated by `,`.
  let mut k = with_at + 5;
  let bytes = text.as_bytes();
  loop {
    while k < cursor_pos && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if upper[k..].starts_with("RECURSIVE ") {
      k += "RECURSIVE ".len();
      while k < cursor_pos && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
    }
    let id_start = k;
    while k < bytes.len() && is_word_char(bytes[k] as char) {
      k += 1;
    }
    let id_end = k;
    if id_end == id_start { return None; }
    if text[id_start..id_end].eq_ignore_ascii_case(name) {
      return Some(TextRange::new((id_start as u32).into(), (id_end as u32).into()));
    }
    // Skip to the matching `)` that closes this binding's body.
    while k < bytes.len() && bytes[k] != b'(' {
      k += 1;
    }
    if k >= bytes.len() { return None; }
    let mut depth = 1i32;
    k += 1;
    while k < bytes.len() && depth > 0 {
      match bytes[k] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        _ => {}
      }
      k += 1;
    }
    while k < bytes.len() && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= bytes.len() || bytes[k] != b',' { return None; }
    k += 1;
  }
}

fn find_alias_site(text: &str, upper: &str, alias: &str) -> Option<TextRange> {
  let bytes = text.as_bytes();
  for kw in ["FROM ", "JOIN ", "UPDATE "] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(kw) {
      let after = from + rel + kw.len();
      // Skip table name.
      let mut i = after;
      while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
      while i < bytes.len() && is_word_char(bytes[i] as char) {
        i += 1;
      }
      // Skip whitespace + optional AS.
      while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
      }
      if upper[i..].starts_with("AS ") {
        i += 3;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
          i += 1;
        }
      }
      // Alias candidate.
      let alias_start = i;
      while i < bytes.len() && is_word_char(bytes[i] as char) {
        i += 1;
      }
      if i > alias_start && text[alias_start..i].eq_ignore_ascii_case(alias) {
        return Some(TextRange::new((alias_start as u32).into(), (i as u32).into()));
      }
      from = after;
    }
  }
  None
}

/// Walk forward inside the body of `CREATE TABLE <left> (...)` to find a
/// column definition whose name is `right`. Returns the range of the
/// column name.
fn column_def_in_create_table(text: &str, left: &str, right: &str) -> Option<TextRange> {
  let upper = text.to_ascii_uppercase();
  for needle in ["CREATE TABLE IF NOT EXISTS ", "CREATE TABLE "] {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(needle) {
      let after = from + rel + needle.len();
      let rest = &text[after..];
      let ws = rest.len() - rest.trim_start().len();
      let name_start = after + ws;
      let mut end = name_start;
      let bytes = text.as_bytes();
      while end < bytes.len() && is_word_char(bytes[end] as char) {
        end += 1;
      }
      if !text[name_start..end].eq_ignore_ascii_case(left) {
        from = after;
        continue;
      }
      // Found the table. Locate `(` ... `)` body.
      let mut i = end;
      while i < bytes.len() && bytes[i] != b'(' {
        i += 1;
      }
      if i >= bytes.len() {
        return None;
      }
      i += 1;
      let body_start = i;
      let mut depth = 1i32;
      while i < bytes.len() && depth > 0 {
        match bytes[i] {
          b'(' => depth += 1,
          b')' => depth -= 1,
          _ => {},
        }
        if depth == 0 {
          break;
        }
        i += 1;
      }
      let body_end = i;
      // Inside body, find column name at start of each entry.
      let body = &text[body_start..body_end];
      for entry_range in split_top_level_commas(body) {
        let entry = body[entry_range.clone()].trim_start();
        let entry_offset_in_body = entry_range.start + (body[entry_range.start..].len() - entry.len());
        // Skip "CONSTRAINT <name>" entries -- not columns.
        let upper_entry = entry.to_ascii_uppercase();
        if upper_entry.starts_with("CONSTRAINT")
          || upper_entry.starts_with("PRIMARY")
          || upper_entry.starts_with("FOREIGN")
          || upper_entry.starts_with("UNIQUE")
          || upper_entry.starts_with("CHECK")
          || upper_entry.starts_with("LIKE")
        {
          continue;
        }
        let ent_bytes = entry.as_bytes();
        let mut e = 0usize;
        while e < ent_bytes.len() && is_word_char(ent_bytes[e] as char) {
          e += 1;
        }
        let col_name = &entry[..e];
        if col_name.eq_ignore_ascii_case(right) {
          let abs_start = body_start + entry_offset_in_body;
          let abs_end = abs_start + e;
          return Some(TextRange::new((abs_start as u32).into(), (abs_end as u32).into()));
        }
      }
      return None;
    }
  }
  None
}

/// Split a comma-separated entry list at top-level commas only (depth-0
/// parens). Returns byte ranges relative to `body`.
fn split_top_level_commas(body: &str) -> Vec<std::ops::Range<usize>> {
  let bytes = body.as_bytes();
  let mut out = Vec::new();
  let mut start = 0usize;
  let mut depth = 0i32;
  for (i, &b) in bytes.iter().enumerate() {
    match b {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(start..i);
        start = i + 1;
      },
      _ => {},
    }
  }
  if start < bytes.len() {
    out.push(start..bytes.len());
  }
  out
}

/// At byte `pos`, return (left, right) of an `a.b` token under the
/// cursor. Returns (Some, None) for a bare word, (Some, Some) for the
/// dotted form, (None, None) when not on a word.
fn split_dotted(text: &str, pos: usize) -> (Option<String>, Option<String>) {
  let bytes = text.as_bytes();
  let pos = pos.min(bytes.len());
  let mut start = pos;
  while start > 0 {
    let c = bytes[start - 1] as char;
    if is_word_char(c) || c == '.' {
      start -= 1;
    } else {
      break;
    }
  }
  let mut end = pos;
  while end < bytes.len() {
    let c = bytes[end] as char;
    if is_word_char(c) || c == '.' {
      end += 1;
    } else {
      break;
    }
  }
  if start == end {
    return (None, None);
  }
  let span = &text[start..end];
  if let Some((l, r)) = span.split_once('.') {
    let l_s = if l.is_empty() { None } else { Some(l.to_string()) };
    let r_s = if r.is_empty() { None } else { Some(r.to_string()) };
    return (l_s, r_s);
  }
  (Some(span.to_string()), None)
}

fn is_word_char(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn to_lsp_range(rope: &Rope, r: TextRange) -> Range {
  let s: u32 = r.start().into();
  let e: u32 = r.end().into();
  Range { start: byte_to_position(rope, s as usize), end: byte_to_position(rope, (e as usize).min(rope.len_bytes())) }
}
fn byte_to_position(rope: &Rope, byte: usize) -> Position {
  let byte = byte.min(rope.len_bytes());
  let line = rope.byte_to_line(byte);
  let line_start_byte = rope.line_to_byte(line);
  let line_slice = rope.line(line);
  let mut utf16 = 0u32;
  let mut bytes_seen = 0usize;
  let bytes_in_line = byte.saturating_sub(line_start_byte);
  for c in line_slice.chars() {
    if bytes_seen >= bytes_in_line {
      break;
    }
    utf16 += c.len_utf16() as u32;
    bytes_seen += c.len_utf8();
  }
  Position { line: line as u32, character: utf16 }
}
