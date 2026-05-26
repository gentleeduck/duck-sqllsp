//! sql042: `UPDATE <table> SET <col> = ...` where `<col>` is not in
//! the target table's catalog definition.
//!
//! Sibling of sql002 (unknown column inside SELECT). UPDATE statements
//! reach the catalog via `UpdateStmt.table` and assignments expose the
//! target column name, so checking the assignments against the
//! catalog's column list is straightforward.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql042"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Update(u) = &stmt.kind else {
      return;
    };
    if u.table.name.is_empty() {
      return;
    }
    let Some(t) = catalog.find_table(u.table.schema.as_deref(), &u.table.name) else {
      // sql001 already covers unresolved table.
      return;
    };
    let mut valid: std::collections::HashSet<String> = t.columns.iter().map(|c| c.name.to_ascii_lowercase()).collect();
    // Source may add columns via ALTER TABLE between the CREATE and
    // the UPDATE; pull those into the valid set too. Lenient scan.
    for col in alter_added_columns(source, &u.table.name) {
      valid.insert(col);
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    for (target, _expr) in &u.assignments {
      // Strip qualifier if present: `t.col` -> `col`.
      let col = target.rsplit('.').next().unwrap_or(target);
      if !valid.contains(&col.to_ascii_lowercase()) {
        // Find `SET ... col` in the source to narrow the range.
        let upper = body.to_ascii_uppercase();
        let set_at = upper.find(" SET ").map(|i| i + 5).unwrap_or(0);
        let target_lower = target.to_ascii_lowercase();
        let body_lower = body.to_ascii_lowercase();
        let range = body_lower[set_at..]
          .find(&target_lower)
          .map(|r| {
            let abs_start = start + set_at + r;
            let abs_end = abs_start + target_lower.len();
            text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into())
          })
          .unwrap_or(stmt.range);
        out.push(Diagnostic {
          code: "sql042",
          severity: Severity::Error,
          message: format!("unknown column `{}` in UPDATE SET (table `{}`)", col, u.table.name),
          range,
        });
      }
    }
  }
}

/// Lenient scan for `ALTER TABLE <table_name> ... ADD COLUMN <col>`.
/// Returns the lower-cased column names found in source. Doesn't
/// distinguish IF NOT EXISTS, schema qualifiers, etc -- catalog
/// already has those, this only supplements.
fn alter_added_columns(source: &str, table: &str) -> Vec<String> {
  let mut out = Vec::new();
  let upper = source.to_ascii_uppercase();
  let bytes = source.as_bytes();
  let n = bytes.len();
  let needle = "ALTER TABLE";
  let table_lc = table.to_ascii_lowercase();
  let mut from = 0usize;
  while let Some(rel) = upper[from..].find(needle) {
    let at = from + rel;
    let mut k = at + needle.len();
    while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
    // Optional ONLY / IF EXISTS prefix.
    for kw in ["ONLY ", "IF EXISTS "] {
      if upper[k..].starts_with(kw) {
        k += kw.len();
        while k < n && bytes[k].is_ascii_whitespace() { k += 1 }
      }
    }
    let id_start = k;
    while k < n && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_' || bytes[k] == b'.' || bytes[k] == b'"') { k += 1 }
    let id = source[id_start..k].trim_matches('"').to_ascii_lowercase();
    let bare = id.rsplit('.').next().unwrap_or(&id);
    from = k;
    if bare != table_lc { continue }
    // Scan `ADD COLUMN <name>` within this ALTER stmt (up to `;`).
    let stmt_end = source[k..].find(';').map(|i| k + i).unwrap_or(n);
    let stmt_body_upper = &upper[k..stmt_end];
    let stmt_body = &source[k..stmt_end];
    let mut local = 0usize;
    while let Some(p_rel) = stmt_body_upper[local..].find("ADD COLUMN") {
      let p = local + p_rel + "ADD COLUMN".len();
      let pb = stmt_body.as_bytes();
      let mut q = p;
      while q < pb.len() && pb[q].is_ascii_whitespace() { q += 1 }
      // Skip optional IF NOT EXISTS.
      if stmt_body_upper[q..].starts_with("IF NOT EXISTS") {
        q += "IF NOT EXISTS".len();
        while q < pb.len() && pb[q].is_ascii_whitespace() { q += 1 }
      }
      let name_start = q;
      while q < pb.len() && (pb[q].is_ascii_alphanumeric() || pb[q] == b'_' || pb[q] == b'"') { q += 1 }
      if q > name_start {
        out.push(stmt_body[name_start..q].trim_matches('"').to_ascii_lowercase());
      }
      local = q;
    }
  }
  out
}
