//! sql757: a partitioned table's PRIMARY KEY does not include every
//! partition key column. PostgreSQL requires every unique constraint
//! (PRIMARY KEY included) on a partitioned table to cover all of the
//! table's partitioning columns -- CREATE TABLE fails with "unique
//! constraint on partitioned table must include all partitioning
//! columns" (0A000) otherwise. Only handles simple column-name
//! partition keys and a single table-level PRIMARY KEY (...) clause;
//! expression partition keys and UNIQUE constraints are out of scope
//! to avoid false positives.

use crate::clause_scan::{find_clause, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql757"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE TABLE") {
      return;
    }
    let Some(part_cols) = partition_key_columns(&upper, body) else { return };
    if part_cols.is_empty() {
      return;
    }
    let Some((pk_start, pk_end, pk_cols)) = table_primary_key(&upper, body) else { return };
    let missing: Vec<&str> =
      part_cols.iter().map(String::as_str).filter(|c| !pk_cols.iter().any(|p| p.eq_ignore_ascii_case(c))).collect();
    if missing.is_empty() {
      return;
    }
    out.push(Diagnostic {
      code: "sql757",
      severity: Severity::Error,
      message: format!(
        "PRIMARY KEY does not include partition key column(s) {} -- PostgreSQL requires every unique constraint on a partitioned table to include all partitioning columns",
        missing.join(", ")
      ),
      range: crate::range_at(start + pk_start, start + pk_end),
    });
  }
}

/// Find `PARTITION BY {RANGE|LIST|HASH} (col[, col...])` and return the
/// column names. `None` if not partitioned, or if any entry is not a
/// bare column reference (an expression key -- out of scope here).
fn partition_key_columns(upper: &str, body: &str) -> Option<Vec<String>> {
  let ub = upper.as_bytes();
  let kw = find_clause(ub, b"PARTITION BY")?;
  let mut i = skip_ws(ub, kw + "PARTITION BY".len());
  for strategy in ["RANGE", "LIST", "HASH"] {
    if upper[i..].starts_with(strategy) {
      i += strategy.len();
      break;
    }
  }
  i = skip_ws(ub, i);
  if ub.get(i) != Some(&b'(') {
    return None;
  }
  let close = match_paren(ub, i)?;
  let list = &body[i + 1..close];
  let mut cols = Vec::new();
  for (entry, _) in split_top_level(list) {
    let (_, name) = parse_simple_ident(entry)?;
    cols.push(name);
  }
  Some(cols)
}

/// Find a top-level `PRIMARY KEY ( col[, col...] )` table constraint
/// inside the CREATE TABLE column-def parens. Returns the constraint's
/// own byte span (for the diagnostic range) and its column names.
fn table_primary_key(upper: &str, body: &str) -> Option<(usize, usize, Vec<String>)> {
  let ub = upper.as_bytes();
  let open = ub.iter().position(|&b| b == b'(')?;
  let close_all = match_paren(ub, open)?;
  let list_up = &upper[open + 1..close_all];
  for (entry, off) in split_top_level(list_up) {
    let trimmed = entry.trim_start();
    let lead_ws = entry.len() - trimmed.len();
    if !trimmed.starts_with("PRIMARY KEY") {
      continue;
    }
    let rest = &trimmed["PRIMARY KEY".len()..];
    let paren_rel = rest.find('(')?;
    let abs_open = open + 1 + off + lead_ws + "PRIMARY KEY".len() + paren_rel;
    let abs_close = match_paren(ub, abs_open)?;
    let cols_src = &body[abs_open + 1..abs_close];
    let mut cols = Vec::new();
    for (c, _) in split_top_level(cols_src) {
      if let Some((_, name)) = parse_simple_ident(c) {
        cols.push(name);
      }
    }
    return Some((open + 1 + off, abs_close + 1, cols));
  }
  None
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
