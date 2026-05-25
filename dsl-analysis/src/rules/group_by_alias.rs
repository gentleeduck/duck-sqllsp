//! sql337: `GROUP BY` references a SELECT-list alias instead of the
//! original column. PG accepts this since 9.0 but the SQL standard
//! says alias names aren't in scope for GROUP BY; many other engines
//! reject it. Hint to use the underlying column expression.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql337"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(sel_at) = upper.find("SELECT ") else { return };
    let select_start = sel_at + 7;
    // Find the FROM keyword at top-level paren depth so we don't pick
    // up the FROM inside `extract(... FROM ...)`.
    let Some(rel) = find_kw_top_level(&body[select_start..], "FROM") else { return };
    let from_at = select_start + rel;
    let select_list = &body[select_start..from_at];
    let mut aliases: Vec<String> = Vec::new();
    for item in split_top_level_commas(select_list) {
      let upper_item = item.to_ascii_uppercase();
      let Some(as_at) = upper_item.rfind(" AS ") else { continue };
      let alias_raw = item[as_at + 4..].trim();
      let alias = alias_raw.trim_matches('"').to_ascii_lowercase();
      if alias.is_empty() || !alias.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { continue }
      aliases.push(alias);
    }
    if aliases.is_empty() { return }
    let Some(rel_gb) = find_kw_top_level(&body[from_at..], "GROUP") else { return };
    let gb_at = from_at + rel_gb;
    // Must be followed by BY (whitespace + BY).
    let after_group = gb_at + "GROUP".len();
    let trimmed = body[after_group..].trim_start();
    if !trimmed.to_ascii_uppercase().starts_with("BY ") { return }
    let by_at = after_group + (body[after_group..].len() - trimmed.len());
    let after = by_at + "BY".len() + 1; // BY + at least one space
    let gb_end = upper[after..]
      .find(|c: char| c == ';')
      .or_else(|| upper[after..].find("ORDER BY"))
      .or_else(|| upper[after..].find("LIMIT"))
      .or_else(|| upper[after..].find("HAVING"))
      .map(|p| after + p)
      .unwrap_or(upper.len());
    let gb_list = &body[after..gb_end];
    for item in split_top_level_commas(gb_list) {
      let tok = item.trim().trim_matches('"').to_ascii_lowercase();
      if aliases.iter().any(|a| a == &tok) {
        // Locate the alias inside the buffer for the diag range.
        let needle_lower = tok.clone();
        let body_lower = body.to_ascii_lowercase();
        let Some(at) = body_lower[after..gb_end].find(&needle_lower) else { return };
        let abs_s = start + after + at;
        let abs_e = abs_s + needle_lower.len();
        out.push(Diagnostic {
          code: "sql337",
          severity: Severity::Hint,
          message: format!("GROUP BY references SELECT-list alias `{tok}` -- SQL-standard portable code groups by the underlying expression"),
          range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
        return;
      }
    }
  }
}

fn find_kw_top_level(s: &str, kw: &str) -> Option<usize> {
  let bytes = s.as_bytes();
  let upper = s.to_ascii_uppercase();
  let upper_bytes = upper.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => { i += 1; while i < bytes.len() && bytes[i] != b'\'' { i += 1 } }
      _ => {}
    }
    if depth == 0 && i + kw.len() <= upper_bytes.len() {
      if &upper_bytes[i..i + kw.len()] == kw.as_bytes() {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let next_ok = i + kw.len() == bytes.len() || !is_word(bytes[i + kw.len()] as char);
        if prev_ok && next_ok { return Some(i) }
      }
    }
    i += 1;
  }
  None
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }

fn split_top_level_commas(s: &str) -> Vec<&str> {
  let bytes = s.as_bytes();
  let mut out = Vec::new();
  let mut start = 0usize;
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => { i += 1; while i < bytes.len() && bytes[i] != b'\'' { i += 1 } }
      b',' if depth == 0 => { out.push(&s[start..i]); start = i + 1 }
      _ => {}
    }
    i += 1;
  }
  out.push(&s[start..]);
  out
}
