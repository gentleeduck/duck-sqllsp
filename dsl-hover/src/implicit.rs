//! Derive implicit column specs that the user did not type but the DB
//! still applies.
//!
//! Examples we surface:
//!   - PRIMARY KEY columns -> NOT NULL + UNIQUE (implicit)
//!   - SERIAL / BIGSERIAL  -> NOT NULL, auto-increment default
//!   - GENERATED ALWAYS AS IDENTITY -> NOT NULL, auto-increment default
//!   - FOREIGN KEY (col) REFERENCES other(col) -> references target
//!
//! Source scan rather than AST walk because dsl-parse's CreateTableStmt
//! doesn't model table-level constraints yet.

use dsl_parse::ColumnDef;

#[derive(Debug, Clone, Default)]
pub struct Implicit {
  pub primary_key: bool,
  pub unique: bool,
  pub auto_increment: bool,
  pub foreign_key: Option<ForeignKey>,
  pub check_count: usize,
}

#[derive(Debug, Clone)]
pub struct ForeignKey {
  pub references: String, // "table(column)" or "schema.table(column)"
}

/// Walk `body_text` (the parenthesised body of the CREATE TABLE) and
/// derive every implicit spec attached to `col` that is not already
/// explicit in its declaration.
pub fn derive(body_text: &str, col: &ColumnDef) -> Implicit {
  let mut out = Implicit::default();
  let name = col.name.as_str();
  let upper_type = col.type_name.to_ascii_uppercase();

  // Inline-PK via the column type? sqlparser sets nullable=false for
  // both NOT NULL and inline PRIMARY KEY, so we can't tell them apart
  // here. Catch it with a text scan instead.
  if column_line_marks_primary(body_text, name) {
    out.primary_key = true;
    out.unique = true;
  }

  // SERIAL / BIGSERIAL / SMALLSERIAL imply NOT NULL + nextval default.
  if matches!(upper_type.as_str(), "SERIAL" | "BIGSERIAL" | "SMALLSERIAL") {
    out.auto_increment = true;
  }
  if column_line_has_identity(body_text, name) {
    out.auto_increment = true;
  }

  // Table-level PRIMARY KEY (col1, col2) clause.
  if table_level_primary_includes(body_text, name) {
    out.primary_key = true;
    out.unique = true;
  }
  if table_level_unique_includes(body_text, name) {
    out.unique = true;
  }
  if let Some(fk) = table_level_fk(body_text, name) {
    out.foreign_key = Some(fk);
  }
  if let Some(fk) = column_level_references(body_text, name) {
    out.foreign_key = Some(fk);
  }
  out.check_count = table_level_check_count(body_text);

  out
}

fn column_line_marks_primary(body: &str, col: &str) -> bool {
  for entry in split_top_level(body) {
    if first_ident(entry).is_some_and(|i| i.eq_ignore_ascii_case(col)) {
      let upper = entry.to_ascii_uppercase();
      if upper.contains("PRIMARY KEY") {
        return true;
      }
    }
  }
  false
}

fn column_line_has_identity(body: &str, col: &str) -> bool {
  for entry in split_top_level(body) {
    if first_ident(entry).is_some_and(|i| i.eq_ignore_ascii_case(col)) {
      let upper = entry.to_ascii_uppercase();
      if upper.contains("GENERATED") && upper.contains("IDENTITY") {
        return true;
      }
    }
  }
  false
}

fn column_level_references(body: &str, col: &str) -> Option<ForeignKey> {
  for entry in split_top_level(body) {
    if !first_ident(entry).is_some_and(|i| i.eq_ignore_ascii_case(col)) {
      continue;
    }
    if let Some(idx) = entry.to_ascii_uppercase().find("REFERENCES") {
      let after = entry[idx + "REFERENCES".len()..].trim_start();
      let target = after.split(|c: char| c.is_whitespace() || c == ',' || c == ')').find(|s| !s.is_empty())?;
      // Try to capture (col) following the target.
      let rest = &after[target.len()..];
      let ref_col = rest.trim_start().strip_prefix('(').and_then(|s| s.find(')').map(|i| s[..i].to_string()));
      let suffix = ref_col.map(|c| format!("({c})")).unwrap_or_default();
      return Some(ForeignKey { references: format!("{target}{suffix}") });
    }
  }
  None
}

fn table_level_primary_includes(body: &str, col: &str) -> bool {
  for entry in split_top_level(body) {
    let upper = entry.to_ascii_uppercase();
    let is_pk = upper.starts_with("PRIMARY KEY") || (upper.starts_with("CONSTRAINT") && upper.contains("PRIMARY KEY"));
    if !is_pk {
      continue;
    }
    if extract_paren_names(entry).iter().any(|c| c.eq_ignore_ascii_case(col)) {
      return true;
    }
  }
  false
}

fn table_level_unique_includes(body: &str, col: &str) -> bool {
  for entry in split_top_level(body) {
    let upper = entry.to_ascii_uppercase();
    let is_uq = upper.starts_with("UNIQUE")
      || (upper.starts_with("CONSTRAINT") && upper.contains("UNIQUE") && !upper.contains("PRIMARY"));
    if !is_uq {
      continue;
    }
    if extract_paren_names(entry).iter().any(|c| c.eq_ignore_ascii_case(col)) {
      return true;
    }
  }
  false
}

fn table_level_fk(body: &str, col: &str) -> Option<ForeignKey> {
  for entry in split_top_level(body) {
    let upper = entry.to_ascii_uppercase();
    let is_fk = upper.contains("FOREIGN KEY") && (upper.starts_with("FOREIGN KEY") || upper.starts_with("CONSTRAINT"));
    if !is_fk {
      continue;
    }
    // FOREIGN KEY (col1, col2) REFERENCES tbl (rcol1, rcol2)
    let cols = extract_paren_names(entry);
    if !cols.iter().any(|c| c.eq_ignore_ascii_case(col)) {
      continue;
    }
    if let Some(idx) = upper.find("REFERENCES") {
      let after = entry[idx + "REFERENCES".len()..].trim_start();
      let target = after.split(|c: char| c.is_whitespace() || c == ',' || c == ')').find(|s| !s.is_empty())?;
      let rest = &after[target.len()..];
      let ref_cols = rest.trim_start().strip_prefix('(').and_then(|s| s.find(')').map(|i| s[..i].to_string()));
      let suffix = ref_cols.map(|c| format!("({c})")).unwrap_or_default();
      return Some(ForeignKey { references: format!("{target}{suffix}") });
    }
  }
  None
}

fn table_level_check_count(body: &str) -> usize {
  split_top_level(body)
    .iter()
    .filter(|e| {
      let u = e.to_ascii_uppercase();
      u.starts_with("CHECK ") || u.starts_with("CHECK(") || (u.starts_with("CONSTRAINT") && u.contains("CHECK"))
    })
    .count()
}

/// Split body text on top-level commas respecting nested parens. Returns
/// trimmed pieces.
fn split_top_level(body: &str) -> Vec<&str> {
  let mut out = Vec::new();
  let mut start = 0usize;
  let mut depth: i32 = 0;
  for (i, ch) in body.char_indices() {
    match ch {
      '(' => depth += 1,
      ')' => depth -= 1,
      ',' if depth == 0 => {
        out.push(body[start..i].trim());
        start = i + 1;
      },
      _ => {},
    }
  }
  out.push(body[start..].trim());
  out
}

fn first_ident(entry: &str) -> Option<&str> {
  entry.split(|c: char| c.is_whitespace() || c == '(' || c == ',').find(|s| !s.is_empty()).map(|s| s.trim_matches('"'))
}

fn extract_paren_names(entry: &str) -> Vec<String> {
  let open = match entry.find('(') {
    Some(i) => i,
    None => return Vec::new(),
  };
  let close = match entry[open..].find(')') {
    Some(i) => open + i,
    None => return Vec::new(),
  };
  entry[open + 1..close].split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect()
}
