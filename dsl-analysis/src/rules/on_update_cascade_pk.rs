//! sql333: `ON UPDATE CASCADE` on a column referenced as a primary key.
//!
//! ON UPDATE CASCADE is rarely the right choice on a PK column --
//! PK values are supposed to be immutable. Almost always means the
//! author confused ON UPDATE with ON DELETE intent. Warn.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql333"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("PRIMARY KEY") { return }
    let Some(on_at) = upper.find("ON UPDATE CASCADE") else { return };
    // Walk back from `ON UPDATE CASCADE` to find the enclosing
    // `FOREIGN KEY (...)` clause; pull the local FK column list.
    // Fire only when one of those columns is ALSO in the table's PK
    // column list. Without this check we flagged every FK ever
    // declared on a table that happens to have a PK, which is every
    // normal child table.
    let fk_cols = preceding_fk_columns(&upper, on_at);
    if fk_cols.is_empty() { return }
    let pk_cols = primary_key_columns(&upper);
    if pk_cols.is_empty() { return }
    let intersects = fk_cols.iter().any(|c| pk_cols.iter().any(|p| p.eq_ignore_ascii_case(c)));
    if !intersects { return }
    let abs_s = start + on_at;
    let abs_e = abs_s + "ON UPDATE CASCADE".len();
    out.push(Diagnostic {
      code: "sql333",
      severity: Severity::Warning,
      message: "ON UPDATE CASCADE on a PRIMARY KEY column is rarely intended -- PK values should be immutable".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

/// Look back from `at` (the `ON UPDATE CASCADE` token in uppercase body)
/// for the enclosing `FOREIGN KEY (col, col)` or column-inline form.
/// Returns the FK column names. Empty vec when none found.
fn preceding_fk_columns(upper: &str, at: usize) -> Vec<String> {
  let prefix = &upper[..at];
  if let Some(fk_at) = prefix.rfind("FOREIGN KEY") {
    let after = &prefix[fk_at + "FOREIGN KEY".len()..];
    if let Some(open) = after.find('(') {
      if let Some(close) = after[open + 1..].find(')') {
        let list = &after[open + 1..open + 1 + close];
        return list.split(',').map(|s| s.trim().trim_matches('"').to_string()).collect();
      }
    }
    return Vec::new();
  }
  // Inline REFERENCES form: column line `col TYPE REFERENCES other(id) ON UPDATE CASCADE`.
  // Walk back skipping balanced parens until hitting a top-level `,`
  // or the CREATE TABLE's outer `(`. That gives us the entry's first
  // token = the column name. Naive line-scan picked the inner paren
  // of `OTHER(ID)` and yielded `ID)` instead of the real column name.
  let bytes = prefix.as_bytes();
  let mut i = prefix.len();
  let mut depth = 0i32;
  let mut entry_start = 0usize;
  while i > 0 {
    let b = bytes[i - 1];
    match b {
      b')' => depth += 1,
      b'(' => {
        if depth == 0 { entry_start = i; break }
        depth -= 1;
      }
      b',' if depth == 0 => { entry_start = i; break }
      _ => {}
    }
    i -= 1;
  }
  let entry = &upper[entry_start..at];
  if !entry.contains("REFERENCES") { return Vec::new() }
  let head = entry.trim_start().split_whitespace().next().unwrap_or("");
  if head.is_empty() { return Vec::new() }
  vec![head.trim_matches('"').to_string()]
}

/// All columns covered by any PRIMARY KEY clause in the upper-cased
/// CREATE TABLE body. Both `col TYPE PRIMARY KEY` and table-level
/// `CONSTRAINT n PRIMARY KEY (a, b)` forms.
fn primary_key_columns(upper: &str) -> Vec<String> {
  let mut out = Vec::new();
  // Table-level `PRIMARY KEY (...)`.
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find("PRIMARY KEY") {
    let at = from + rel;
    let after = &upper[at + "PRIMARY KEY".len()..];
    let trimmed = after.trim_start();
    if trimmed.starts_with('(') {
      let inner_start = after.len() - trimmed.len() + 1;
      if let Some(close) = after[inner_start..].find(')') {
        let list = &after[inner_start..inner_start + close];
        for c in list.split(',') {
          out.push(c.trim().trim_matches('"').to_string());
        }
      }
    } else {
      // Inline: column NAME ... PRIMARY KEY. Walk back to find the col name.
      let prefix = &upper[..at];
      let line_start = prefix.rfind(|c: char| c == ',' || c == '\n' || c == '(').map(|p| p + 1).unwrap_or(0);
      let line = &upper[line_start..at];
      let head = line.trim_start().split_whitespace().next().unwrap_or("");
      if !head.is_empty() { out.push(head.trim_matches('"').to_string()); }
    }
    from = at + "PRIMARY KEY".len();
  }
  out
}
