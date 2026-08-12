//! MERGE statement completion.
//!
//! Detects the cursor position within a `MERGE INTO <target> USING
//! <source> ON <cond> WHEN [NOT] MATCHED [AND <cond>] THEN <action>`
//! statement and returns the right slot classification / next-keyword
//! set. Extracted from `engine.rs` (was inline there) -- pure code
//! motion, no behavior change.

use crate::engine::{cursor_not_at_ws_boundary, stmt_slice_upper};
use text_size::TextSize;

pub fn merge_update_set_lhs_slot(source: &str, offset: TextSize) -> bool {
  if cursor_not_at_ws_boundary(source, offset) {
    return false;
  }
  let (slice, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"MERGE") {
    return false;
  }
  let set_idx = match words.iter().rposition(|w| *w == "SET") {
    Some(i) => i,
    None => return false,
  };
  let after_set = &words[set_idx + 1..];
  if after_set.is_empty() {
    return true;
  }
  let trimmed = slice.trim_end();
  trimmed.ends_with(',')
}

pub fn merge_update_set_rhs_expr_slot(source: &str, offset: TextSize) -> bool {
  if cursor_not_at_ws_boundary(source, offset) {
    return false;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"MERGE") {
    return false;
  }
  let set_idx = match words.iter().rposition(|w| *w == "SET") {
    Some(i) => i,
    None => return false,
  };
  let after_set = &words[set_idx + 1..];
  after_set.contains(&"=")
}

/// True when the cursor sits at `MERGE ... WHEN NOT MATCHED THEN
/// INSERT (<cursor>` -- the column-list slot. Detect by walking back
/// to an unmatched `(` whose preceding word is INSERT, with MERGE as
/// the statement-leading keyword.
pub fn merge_insert_col_list_slot(source: &str, offset: TextSize) -> bool {
  let pos: usize = (u32::from(offset) as usize).min(source.len());
  let bytes = source.as_bytes();
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"MERGE") {
    return false;
  }
  let mut depth = 0i32;
  let mut i = pos;
  while i > 0 {
    i -= 1;
    match bytes[i] {
      b')' => depth += 1,
      b'(' => {
        if depth == 0 {
          break;
        }
        depth -= 1;
      },
      _ => {},
    }
  }
  if bytes.get(i) != Some(&b'(') {
    return false;
  }
  let mut e = i;
  while e > 0 && bytes[e - 1].is_ascii_whitespace() {
    e -= 1;
  }
  let mut s = e;
  while s > 0 && (bytes[s - 1].is_ascii_alphanumeric() || bytes[s - 1] == b'_') {
    s -= 1;
  }
  s != e && source[s..e].eq_ignore_ascii_case("INSERT")
}

/// True when the cursor sits at `MERGE ... WHEN [NOT] MATCHED AND
/// <cursor>` -- the predicate slot. Returns false when AND is absent
/// or the statement isn't a MERGE.
pub fn merge_when_matched_and_predicate_slot(source: &str, offset: TextSize) -> bool {
  if cursor_not_at_ws_boundary(source, offset) {
    return false;
  }
  let (_, upper) = stmt_slice_upper(source, offset);
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"MERGE") {
    return false;
  }
  let n = words.len();
  if n < 2 || words[n - 1] != "AND" {
    return false;
  }
  let mut i = n - 1;
  while i > 0 {
    i -= 1;
    if words[i] == "THEN" {
      return false;
    }
    if words[i] == "MATCHED" {
      return true;
    }
  }
  false
}

/// Parse `MERGE INTO <target> [AS alias_t] USING <source> [AS alias_s]`
/// from the buffer and return (target, source) table names. Crude
/// whitespace tokenization; preserves original case.
pub fn merge_target_and_source(source: &str) -> (Option<String>, Option<String>) {
  let upper = source.to_ascii_uppercase();
  let into = upper.find("MERGE INTO ");
  let using = upper.find(" USING ");
  let into_pos = match into {
    Some(p) => p + "MERGE INTO ".len(),
    None => return (None, None),
  };
  let using_pos = match using {
    Some(p) => p,
    None => return (None, None),
  };
  if using_pos <= into_pos {
    return (None, None);
  }
  let target = extract_first_ident(&source[into_pos..using_pos]);
  let after_using = &source[using_pos + " USING ".len()..];
  let on_at = after_using.to_ascii_uppercase().find(" ON ").unwrap_or(after_using.len());
  let src = extract_first_ident(&after_using[..on_at]);
  (target, src)
}

/// Extract `MERGE INTO <t> <alias_t>, USING <s> <alias_s>` aliases
/// (the bare identifiers right after the table names).
pub fn merge_aliases(source: &str) -> Vec<String> {
  let mut out = Vec::new();
  let upper = source.to_ascii_uppercase();
  if let Some(p) = upper.find("MERGE INTO ") {
    let rest = &source[p + "MERGE INTO ".len()..];
    if let Some(alias) = nth_ident(rest, 1) {
      out.push(alias);
    }
  }
  if let Some(p) = upper.find(" USING ") {
    let rest = &source[p + " USING ".len()..];
    if let Some(alias) = nth_ident(rest, 1) {
      out.push(alias);
    }
  }
  out
}

fn extract_first_ident(s: &str) -> Option<String> {
  nth_ident(s, 0)
}

/// Pick the n-th whitespace-separated bare identifier (alphanumeric +
/// underscore + dot). Skips the optional `AS` keyword.
fn nth_ident(s: &str, n: usize) -> Option<String> {
  let mut idx = 0;
  for tok in s.split_ascii_whitespace() {
    let t = tok.trim_end_matches([',', ';']);
    if t.eq_ignore_ascii_case("AS") {
      continue;
    }
    if !t.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false) {
      return None;
    }
    let ident: String = t.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.').collect();
    if ident.is_empty() {
      return None;
    }
    if idx == n {
      return Some(ident.rsplit('.').next().unwrap_or(&ident).to_string());
    }
    idx += 1;
  }
  None
}

pub fn merge_next_keyword(source: &str, offset: TextSize) -> Option<&'static [(&'static str, &'static str)]> {
  if cursor_not_at_ws_boundary(source, offset) {
    return None;
  }
  let (slice_owned, _) = stmt_slice_upper(source, offset);
  let slice = slice_owned.trim();
  let upper = slice.to_ascii_uppercase();
  let words: Vec<&str> = upper.split_ascii_whitespace().collect();
  if words.first() != Some(&"MERGE") {
    return None;
  }
  let n = words.len();
  if n == 1 {
    return Some(&[("INTO", "MERGE INTO <target_table>")]);
  }
  if n >= 3
    && words[1] == "INTO"
    && !words[n - 1].chars().all(|c| c.is_ascii_uppercase())
    && !["USING", "AS", "ON", "WHEN"].contains(&words[n - 1])
  {
    return Some(&[("USING", "USING <source_table_or_subquery>"), ("AS", "AS <alias>")]);
  }
  if words.contains(&"USING") && !words.contains(&"ON") && !words.contains(&"WHEN") {
    let after_using_idx = words.iter().position(|w| *w == "USING").unwrap();
    if n > after_using_idx + 1 {
      return Some(&[("ON", "ON <join_condition>")]);
    }
  }
  if words.contains(&"ON") && !words.contains(&"WHEN") {
    return Some(&[("WHEN", "WHEN [NOT] MATCHED [AND ...] THEN ...")]);
  }
  if matches!(words.last(), Some(&"WHEN")) {
    return Some(&[("MATCHED", "WHEN MATCHED [AND ...] THEN ..."), ("NOT MATCHED", "WHEN NOT MATCHED [AND ...] THEN ...")]);
  }
  if matches!(words.last(), Some(&"MATCHED")) && !words.contains(&"THEN") {
    return Some(&[("THEN", "THEN <action>"), ("AND", "AND <extra_condition>")]);
  }
  if let Some(then_idx) = words.iter().rposition(|w| *w == "THEN")
    && then_idx == n - 1
  {
    let prior = &words[..then_idx];
    let is_not_matched = prior.windows(2).any(|w| w[0] == "NOT" && w[1] == "MATCHED");
    if is_not_matched {
      return Some(&[("INSERT", "INSERT (<cols>) VALUES (<vals>)"), ("DO NOTHING", "DO NOTHING")]);
    }
    return Some(&[("UPDATE", "UPDATE SET <col> = <val> [, ...]"), ("DELETE", "DELETE"), ("DO NOTHING", "DO NOTHING")]);
  }
  if matches!(words.last(), Some(&"UPDATE"))
    && words.iter().rposition(|w| *w == "THEN").map(|t| t == n - 2).unwrap_or(false)
  {
    return Some(&[("SET", "SET <col> = <val> [, <col> = <val> ...]")]);
  }
  if matches!(words.last(), Some(&"INSERT"))
    && words.iter().rposition(|w| *w == "THEN").map(|t| t == n - 2).unwrap_or(false)
  {
    return Some(&[
      ("VALUES", "VALUES (<v1>, <v2>, ...)"),
      ("OVERRIDING", "OVERRIDING { SYSTEM | USER } VALUE"),
      ("DEFAULT VALUES", "DEFAULT VALUES"),
      ("(", "( <col1>, <col2>, ... ) VALUES (...)"),
    ]);
  }
  None
}
