//! sql239: `ALTER TABLE t DROP COLUMN c` where `c` was declared in
//! a `CREATE TABLE t (... c ...)` earlier in the same buffer. The
//! migration cancels itself; the author probably meant to drop the
//! create-table column instead.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql239"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("ALTER TABLE") { return }
    if !upper.contains("DROP COLUMN") { return }
    // Extract table + column.
    let Some(at_at) = upper.find("ALTER TABLE") else { return };
    let after = at_at + "ALTER TABLE".len();
    let rest = body[after..].trim_start();
    let id_end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(rest.len());
    let table_raw = &rest[..id_end];
    let table = table_raw.rsplit('.').next().unwrap_or(table_raw).trim_matches('"').to_string();
    let Some(drop_at) = upper.find("DROP COLUMN") else { return };
    let after_drop = drop_at + "DROP COLUMN".len();
    let drop_rest = body[after_drop..].trim_start();
    let col_end = drop_rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '"').unwrap_or(drop_rest.len());
    let col = drop_rest[..col_end].trim_matches('"').to_string();
    if col.is_empty() || table.is_empty() { return }
    // Walk the prelude for a matching CREATE TABLE with this column.
    let prelude_upper = source[..start].to_ascii_uppercase();
    if !prelude_upper.contains(&format!("CREATE TABLE {}", table.to_ascii_uppercase()))
      && !prelude_upper.contains(&format!("CREATE TABLE IF NOT EXISTS {}", table.to_ascii_uppercase()))
    { return }
    // Find the matching CREATE body and check the column is there.
    if !column_in_create(&source[..start], &table, &col) { return }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql239",
      severity: Severity::Hint,
      message: format!(
        "ALTER TABLE `{table}` DROP COLUMN `{col}` cancels the column declared in CREATE TABLE above -- just remove it from the CREATE TABLE column list"
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn column_in_create(prefix: &str, table: &str, col: &str) -> bool {
  let upper = prefix.to_ascii_uppercase();
  for needle in [
    format!("CREATE TABLE {} (", table.to_ascii_uppercase()),
    format!("CREATE TABLE IF NOT EXISTS {} (", table.to_ascii_uppercase()),
  ] {
    if let Some(at) = upper.find(&needle) {
      let open = at + needle.len() - 1;
      let Some(close) = find_matching_paren(prefix, open) else { continue };
      let body = &prefix[open + 1..close];
      let body_upper = body.to_ascii_uppercase();
      let needle_col = format!("{} ", col.to_ascii_uppercase());
      for line in body_upper.split(',') {
        let t = line.trim_start();
        if t.starts_with(&needle_col) || t.starts_with(&format!("\"{}\"", col.to_ascii_uppercase())) {
          return true;
        }
      }
    }
  }
  false
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
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
