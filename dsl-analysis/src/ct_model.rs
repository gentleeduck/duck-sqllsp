//! Effective column model for CREATE TABLE statements.
//!
//! Many rules want to ask "is this column nullable / is it the primary
//! key / what's its effective default" -- but the answer depends on a
//! mix of column-level syntax AND table-level constraints AND implicit
//! Postgres semantics (PK → NOT NULL, SERIAL → NOT NULL + default,
//! IDENTITY → NOT NULL, etc).
//!
//! This module folds all of that into one pass so rules can read
//! `effective_columns(body)` and forget about the lexical surface.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectiveCol {
  pub name: String,
  pub data_type: String,
  pub nullable: bool,
  pub is_primary_key: bool,
  pub is_unique: bool,
  pub is_serial: bool,
  pub is_identity: bool,
  pub has_default: bool,
}

/// Parse the column list of a CREATE TABLE statement and apply
/// Postgres' implicit semantics: PK ⇒ NOT NULL, SERIAL ⇒ NOT NULL +
/// has_default, IDENTITY ⇒ NOT NULL, table-level PRIMARY KEY (cols)
/// marks each referenced column as PK + NOT NULL.
pub fn effective_columns(body: &str) -> Vec<EffectiveCol> {
  let upper = body.to_ascii_uppercase();
  let Some(open) = body.find('(') else { return Vec::new() };
  let bytes = body.as_bytes();
  let n = bytes.len();
  let mut depth = 1i32;
  let mut end = open + 1;
  while end < n && depth > 0 {
    match bytes[end] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        end += 1;
        while end < n && bytes[end] != b'\'' {
          end += 1;
        }
      },
      _ => {},
    }
    if depth == 0 {
      break;
    }
    end += 1;
  }
  let list = &body[open + 1..end];
  let list_up = &upper[open + 1..end];
  let mut cols: Vec<EffectiveCol> = Vec::new();
  let mut tbl_pk_cols: Vec<String> = Vec::new();
  let mut tbl_unique_cols: Vec<String> = Vec::new();
  // Walk top-level entries (column defs + table-level constraints).
  let lb = list.as_bytes();
  let ln = list.len();
  let mut d = 0i32;
  let mut start = 0usize;
  let mut idx = 0usize;
  while idx <= ln {
    let at_end = idx == ln;
    let c = if at_end { b',' } else { lb[idx] };
    match c {
      b'(' => {
        d += 1;
        idx += 1;
        continue;
      },
      b')' => {
        d -= 1;
        idx += 1;
        continue;
      },
      b'\'' if !at_end => {
        idx += 1;
        while idx < ln && lb[idx] != b'\'' {
          idx += 1;
        }
        if idx < ln {
          idx += 1;
        }
        continue;
      },
      _ => {},
    }
    if c == b',' && d == 0 {
      let chunk = &list[start..idx];
      let chunk_up = &list_up[start..idx];
      classify_entry(chunk, chunk_up, &mut cols, &mut tbl_pk_cols, &mut tbl_unique_cols);
      start = idx + 1;
    }
    if at_end {
      break;
    }
    idx += 1;
  }
  // Apply table-level PRIMARY KEY: each referenced column becomes
  // is_primary_key + nullable=false.
  for pk in &tbl_pk_cols {
    if let Some(col) = cols.iter_mut().find(|c| c.name.eq_ignore_ascii_case(pk)) {
      col.is_primary_key = true;
      col.nullable = false;
    }
  }
  for u in &tbl_unique_cols {
    if let Some(col) = cols.iter_mut().find(|c| c.name.eq_ignore_ascii_case(u)) {
      col.is_unique = true;
    }
  }
  cols
}

fn classify_entry(
  chunk: &str,
  chunk_up: &str,
  cols: &mut Vec<EffectiveCol>,
  tbl_pk_cols: &mut Vec<String>,
  tbl_unique_cols: &mut Vec<String>,
) {
  let trimmed_up = chunk_up.trim_start();
  let trimmed = chunk.trim_start();
  // Table-level PRIMARY KEY ( cols )
  if trimmed_up.starts_with("PRIMARY KEY") {
    if let Some(open) = trimmed.find('(') {
      if let Some(close) = trimmed[open + 1..].find(')') {
        let list = &trimmed[open + 1..open + 1 + close];
        for c in list.split(',') {
          tbl_pk_cols.push(c.trim().trim_matches('"').to_string());
        }
      }
    }
    return;
  }
  if trimmed_up.starts_with("UNIQUE") {
    if let Some(open) = trimmed.find('(') {
      if let Some(close) = trimmed[open + 1..].find(')') {
        let list = &trimmed[open + 1..open + 1 + close];
        for c in list.split(',') {
          tbl_unique_cols.push(c.trim().trim_matches('"').to_string());
        }
      }
    }
    return;
  }
  if trimmed_up.starts_with("CONSTRAINT") {
    // Named constraint -- strip CONSTRAINT <name> then re-dispatch.
    let rest = trimmed.split_whitespace().skip(2).collect::<Vec<_>>().join(" ");
    let rest_up = rest.to_ascii_uppercase();
    if rest_up.starts_with("PRIMARY KEY") || rest_up.starts_with("UNIQUE") {
      return classify_entry(&rest, &rest_up, cols, tbl_pk_cols, tbl_unique_cols);
    }
    return;
  }
  if trimmed_up.starts_with("FOREIGN")
    || trimmed_up.starts_with("CHECK")
    || trimmed_up.starts_with("EXCLUDE")
    || trimmed_up.starts_with("LIKE")
  {
    return;
  }
  // Column definition.
  if trimmed.is_empty() {
    return;
  }
  let name: String = trimmed.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '"').collect();
  if name.is_empty() {
    return;
  }
  let name = name.trim_matches('"').to_string();
  // Detect type token (next word after the name).
  let after_name = trimmed[name.len()..].trim_start();
  let data_type: String = after_name.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
  let dt_up = data_type.to_ascii_uppercase();
  let is_serial = matches!(dt_up.as_str(), "SERIAL" | "BIGSERIAL" | "SMALLSERIAL");
  let is_identity = chunk_up.contains("GENERATED") && chunk_up.contains("IDENTITY");
  let has_not_null = chunk_up.contains("NOT NULL") || chunk_up.contains("PRIMARY KEY") || is_serial || is_identity;
  let has_default = chunk_up.contains("DEFAULT") || is_serial || chunk_up.contains("GENERATED");
  let is_primary_key = chunk_up.contains("PRIMARY KEY");
  let is_unique = chunk_up.contains("UNIQUE");
  cols.push(EffectiveCol {
    name,
    data_type,
    nullable: !has_not_null,
    is_primary_key,
    is_unique,
    is_serial,
    is_identity,
    has_default,
  });
}
