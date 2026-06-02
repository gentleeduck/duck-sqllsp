//! sql269: `WHERE EXTRACT(YEAR FROM ts) = 2024` or
//! `WHERE date_part('year', ts) = 2024` -- wrapping a timestamp
//! column in EXTRACT / date_part prevents the planner from using a
//! btree index. Suggest a range predicate
//! (`ts >= '2024-01-01' AND ts < '2025-01-01'`) so the index applies.
//!
//! Skip when the operand isn't a real column (e.g. CURRENT_DATE,
//! now()) -- no index to block in that case.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql269"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("WHERE") {
      return;
    }
    // EXTRACT(unit FROM operand) form.
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("EXTRACT(") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 8;
          continue;
        }
      }
      let open = at + "EXTRACT(".len() - 1;
      let Some(close) = find_matching_paren(body, open) else { break };
      let post = body[close + 1..].trim_start();
      if !post.starts_with('=') {
        from = close + 1;
        continue;
      }
      // Inspect the operand: must be a column-shaped expression
      // (a bare ident or `qualifier.ident`). Skip CURRENT_*, now(),
      // and anything containing parens or commas.
      let inner = &body[open + 1..close];
      if !extract_from_is_column(inner) {
        from = close + 1;
        continue;
      }
      out.push(Diagnostic {
        code: "sql269",
        severity: Severity::Hint,
        message: "EXTRACT(... FROM col) = N blocks btree index on col -- prefer a range predicate (e.g. col >= 'YYYY-01-01' AND col < 'YYYY+1-01-01')".into(),
        range: crate::range_at(start + at, start + close + 1),
      });
      from = close + 1;
    }
    // date_part('unit', operand) form -- same semantic shape.
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("DATE_PART(") {
      let at = from + rel;
      if at > 0 {
        let prev = body.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = at + 10;
          continue;
        }
      }
      let open = at + "DATE_PART(".len() - 1;
      let Some(close) = find_matching_paren(body, open) else { break };
      let post = body[close + 1..].trim_start();
      if !post.starts_with('=') {
        from = close + 1;
        continue;
      }
      let inner = &body[open + 1..close];
      // date_part takes (text, source) -- look at the second arg.
      let Some(second) = top_level_second_arg(inner) else {
        from = close + 1;
        continue;
      };
      if !date_part_operand_is_column(second) {
        from = close + 1;
        continue;
      }
      out.push(Diagnostic {
        code: "sql269",
        severity: Severity::Hint,
        message: "date_part('unit', col) = N blocks btree index on col -- prefer a range predicate (e.g. col >= 'YYYY-01-01' AND col < 'YYYY+1-01-01')".into(),
        range: crate::range_at(start + at, start + close + 1),
      });
      from = close + 1;
    }
  }
}

/// True when the EXTRACT(... FROM <expr>) operand is a bare column
/// reference (`col` or `qualifier.col`). Returns false for
/// CURRENT_*, function calls, casts, literals.
fn extract_from_is_column(inner: &str) -> bool {
  let upper = inner.to_ascii_uppercase();
  let Some(idx) = upper.find(" FROM ") else {
    return false;
  };
  let operand = inner[idx + " FROM ".len()..].trim();
  is_column_shape(operand)
}

fn date_part_operand_is_column(s: &str) -> bool {
  is_column_shape(s.trim())
}

/// True when `s` looks like a bare column reference -- only word
/// characters + at most one `.`, and not a recognized non-column
/// keyword (CURRENT_*, NULL, TRUE, FALSE).
fn is_column_shape(s: &str) -> bool {
  if s.is_empty() {
    return false;
  }
  for c in s.chars() {
    if !(c.is_alphanumeric() || c == '_' || c == '.') {
      return false;
    }
  }
  let up = s.to_ascii_uppercase();
  let bare = up.rsplit('.').next().unwrap_or(&up);
  if matches!(bare, "CURRENT_DATE" | "CURRENT_TIME" | "CURRENT_TIMESTAMP" | "LOCALTIME" | "LOCALTIMESTAMP" | "NOW" | "NULL" | "TRUE" | "FALSE") {
    return false;
  }
  // Bare digits are literals.
  if bare.chars().all(|c| c.is_ascii_digit() || c == '.') {
    return false;
  }
  true
}

fn top_level_second_arg(s: &str) -> Option<&str> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  let mut commas: Vec<usize> = Vec::new();
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => commas.push(i),
      _ => {},
    }
    i += 1;
  }
  if commas.len() != 1 {
    return None;
  }
  Some(&s[commas[0] + 1..])
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
