//! sql345: `ALTER TABLE t RENAME COLUMN old TO new` while some
//! `CREATE VIEW v AS SELECT ...` in the same buffer references both
//! table `t` and column name `old`. PG cascades the rename for views
//! defined with an explicit column list, but inline `SELECT old`
//! references silently become invalid and the view stops compiling on
//! next pg_dump / DEFINITION refresh.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql345"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(at) = upper.find("RENAME COLUMN ") else { return };
    // Need: ALTER TABLE <t> RENAME COLUMN <old> TO <new>
    let Some(alter_at) = upper.find("ALTER TABLE") else { return };
    let after_alter = alter_at + "ALTER TABLE".len();
    let rest = &body[after_alter..];
    let lead = rest.len() - rest.trim_start().len();
    let raw = &rest[lead..];
    let tbl_end =
      raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '"').unwrap_or(raw.len());
    let table = raw[..tbl_end].rsplit('.').next().unwrap_or(&raw[..tbl_end]).trim_matches('"').to_ascii_lowercase();
    if table.is_empty() {
      return;
    }
    // Old column name.
    let after_rc = at + "RENAME COLUMN ".len();
    let rc_rest = &body[after_rc..];
    let rc_lead = rc_rest.len() - rc_rest.trim_start().len();
    let rc_raw = &rc_rest[rc_lead..];
    let old_end = rc_raw.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '"').unwrap_or(rc_raw.len());
    let old_col = rc_raw[..old_end].trim_matches('"').to_ascii_lowercase();
    if old_col.is_empty() {
      return;
    }
    // Scan the whole buffer for affected views.
    let affected = find_views_referencing(source, &table, &old_col);
    if affected.is_empty() {
      return;
    }
    let abs_s = start + at;
    let abs_e = abs_s + "RENAME COLUMN".len();
    out.push(Diagnostic {
      code: "sql345",
      severity: Severity::Warning,
      message: format!(
        "RENAME COLUMN `{}.{}` -- affects view(s): {}; reissue CREATE OR REPLACE VIEW after the rename",
        table,
        old_col,
        affected.join(", ")
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

/// Walks every `CREATE [OR REPLACE] VIEW <name> AS ...;` in `source`.
/// A view "references" the rename when its body contains both the
/// table name (any dotted form) AND the old column name as whole
/// words.
fn find_views_referencing(source: &str, table: &str, old_col: &str) -> Vec<String> {
  let upper = source.to_ascii_uppercase();
  let bytes = source.as_bytes();
  let mut out = Vec::new();
  for needle in
    ["CREATE OR REPLACE VIEW ", "CREATE VIEW ", "CREATE MATERIALIZED VIEW ", "CREATE OR REPLACE MATERIALIZED VIEW "]
  {
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(needle) {
      let after = from + rel + needle.len();
      let mut k = after;
      while k < bytes.len() && bytes[k].is_ascii_whitespace() {
        k += 1
      }
      let name_start = k;
      while k < bytes.len()
        && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'"' || bytes[k] == b'.')
      {
        k += 1;
      }
      let view_name = source[name_start..k].trim_matches('"').to_string();
      let stmt_end = source[after..].find(';').map(|i| after + i).unwrap_or(source.len());
      let body = &source[after..stmt_end];
      if word_contains(body, table) && word_contains(body, old_col) && !out.contains(&view_name) {
        out.push(view_name);
      }
      from = stmt_end + 1;
    }
  }
  out
}

fn word_contains(haystack: &str, needle: &str) -> bool {
  let h = haystack.as_bytes();
  let n = needle.as_bytes();
  if n.is_empty() {
    return false;
  }
  let mut i = 0usize;
  while i + n.len() <= h.len() {
    if h[i..i + n.len()].eq_ignore_ascii_case(n) {
      let prev_ok = i == 0 || !is_word(h[i - 1] as char);
      let next_ok = i + n.len() == h.len() || !is_word(h[i + n.len()] as char);
      if prev_ok && next_ok {
        return true;
      }
    }
    i += 1;
  }
  false
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
