//! CREATE INDEX sub-phase detector.
//!
//! Recognises `CREATE [UNIQUE] INDEX [CONCURRENTLY] [IF NOT EXISTS] <name>
//! ON <table> (<expr>, ...)` and returns the right [`Phase`] depending on
//! cursor position:
//!
//! - right after `ON ` -> `ExpectTable` (catalog table list)
//! - inside the paren body -> `CtlExpectFkColumn { table }` so only columns
//!   of THIS table appear

use crate::phase::Phase;
use text_size::TextSize;

pub fn detect(source: &str, offset: TextSize) -> Option<Phase> {
  let pos: usize = u32::from(offset) as usize;
  let pos = pos.min(source.len());

  // Find the most recent CREATE INDEX header before the cursor.
  let upper = source.to_ascii_uppercase();
  let mut header: Option<usize> = None;
  for needle in ["CREATE UNIQUE INDEX ", "CREATE INDEX "] {
    let mut i = 0usize;
    while let Some(rel) = upper[i..].find(needle) {
      let p = i + rel;
      if p > pos {
        break;
      }
      header = Some(p);
      i = p + needle.len();
    }
    if header.is_some() {
      break;
    }
  }
  let start = header?;
  // Ensure the header isn't already terminated by `;` before the cursor.
  if source[start..pos].contains(';') {
    return None;
  }

  // Find the `ON` keyword between start and cursor.
  let slice = &source[start..pos];
  let su = slice.to_ascii_uppercase();
  let on_rel = find_kw(&su, "ON")?;
  let after_on = on_rel + 2;
  let rest = &slice[after_on..];

  // Read the table name token after ON.
  let trim = rest.len() - rest.trim_start().len();
  let table_start = trim;
  let table: String =
    rest[table_start..].chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
  if table.is_empty() {
    // Cursor sits between `ON ` and any name -> suggest tables.
    return Some(Phase::ExpectTable);
  }
  let after_table = table_start + table.len();

  // Past the table -- skip an optional `USING <method>` clause so
  // `CREATE INDEX ix ON users USING btree (` still routes the cursor
  // inside the paren body to the table-column slot.
  let mut after = rest[after_table..].trim_start();
  let after_upper = after.to_ascii_uppercase();
  if after_upper.starts_with("USING") {
    let bytes = after.as_bytes();
    let n = bytes.len();
    let kw_ok = n == 5 || !(bytes[5].is_ascii_alphanumeric() || bytes[5] == b'_');
    if kw_ok {
      let rest_after_using = after[5..].trim_start();
      // Consume the method name (single word identifier).
      let method_len: usize =
        rest_after_using.chars().take_while(|c| c.is_alphanumeric() || *c == '_').map(|c| c.len_utf8()).sum();
      after = rest_after_using[method_len..].trim_start();
    }
  }
  if !after.starts_with('(') {
    // Still typing the table name -> tables.
    if rest[table_start..after_table].len() == rest[table_start..].trim_end().len() {
      return Some(Phase::ExpectTable);
    }
    return None;
  }
  // Walk the paren body. When it closes BEFORE the cursor the user is in
  // the trailing-clause slot (INCLUDE / WHERE / WITH / ...), which is
  // handled by the engine-level keyword fallback -- return None here so
  // that path can fire.
  let body_bytes = after.as_bytes();
  let mut depth: i32 = 0;
  let mut k = 0usize;
  while k < body_bytes.len() {
    match body_bytes[k] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          // Cursor sits past the closing paren -> trailing-clause slot.
          return None;
        }
      },
      _ => {},
    }
    k += 1;
  }
  // Paren still open -> cursor sits inside the index expression list.
  let name = table.rsplit('.').next().unwrap_or(&table).to_string();
  Some(Phase::CtlExpectFkColumn { table: name })
}

/// Whole-word match of `kw` in `upper`. Returns its byte offset or None.
fn find_kw(upper: &str, kw: &str) -> Option<usize> {
  let bytes = upper.as_bytes();
  let n = bytes.len();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find(kw) {
    let i = from + rel;
    let after = i + kw.len();
    let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
    let next_ok = after >= n || !(bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
    if prev_ok && next_ok {
      return Some(i);
    }
    from = after;
  }
  None
}
