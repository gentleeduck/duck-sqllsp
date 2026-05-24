//! CREATE INDEX sub-phase detector.
//!
//! Recognises `CREATE [UNIQUE] INDEX [CONCURRENTLY] [IF NOT EXISTS] <name>
//! ON <table> (<expr>, ...)` and returns the right [`Phase`] depending on
//! cursor position:
//!
//!   - right after `ON `      -> `ExpectTable` (catalog table list)
//!   - inside the paren body  -> `CtlExpectFkColumn { table }`
//!                                so only columns of THIS table appear

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

  // Past the table -- are we inside the paren body?
  let after = rest[after_table..].trim_start();
  if !after.starts_with('(') {
    // Still typing the table name -> tables.
    // (If the user finished typing and a `(` is expected next, the
    // generic keyword set kicks in -- nothing useful to suggest.)
    if rest[table_start..after_table].len() == rest[table_start..].trim_end().len() {
      return Some(Phase::ExpectTable);
    }
    return None;
  }
  // Cursor sits inside the index expression list. Columns of the table.
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
