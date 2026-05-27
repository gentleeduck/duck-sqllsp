//! sql508: `WHERE col LIKE col` / `ILIKE` / `NOT LIKE` / `NOT ILIKE`
//! and the POSIX-regex equivalents `~ / ~* / !~ / !~*` -- a column
//! compared against itself is almost always a copy-paste typo for
//! two distinct columns. The expression is also semantically
//! degenerate: `col LIKE col` is TRUE for every non-NULL row
//! regardless of pattern (and NULL for NULL rows), so the predicate
//! has no filter effect. `NOT LIKE` is the always-FALSE inverse.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql508"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "HAVING", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    let Some(rel_where) = find_clause(ub, b"WHERE") else { return };
    let pred_start = rel_where + 5;
    let pred_end = find_clause_end(ub, pred_start, &stopwords).min(ub.len());

    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    // Two paths: word operators (LIKE/ILIKE, optionally preceded by NOT),
    // and symbol operators (~ / ~* / !~ / !~*).
    let mut i = pred_start;
    while i < pred_end {
      // Try LIKE / ILIKE / NOT LIKE / NOT ILIKE first.
      if let Some((op_text, op_start, op_end)) = try_word_op(ub, i, pred_end) {
        if let Some((lhs, rhs, abs_s, abs_e)) = extract_lhs_rhs(raw, bytes, op_start, op_end, pred_start, pred_end)
          && idents_eq(lhs, rhs)
          && emitted.insert(op_start)
        {
          push_diag(start, abs_s, abs_e, &op_text, lhs, out);
        }
        i = op_end;
        continue;
      }
      // Symbol operators.
      if let Some((op_text, op_start, op_end)) = try_symbol_op(bytes, i, pred_end) {
        if let Some((lhs, rhs, abs_s, abs_e)) = extract_lhs_rhs(raw, bytes, op_start, op_end, pred_start, pred_end)
          && idents_eq(lhs, rhs)
          && emitted.insert(op_start)
        {
          push_diag(start, abs_s, abs_e, &op_text, lhs, out);
        }
        i = op_end;
        continue;
      }
      i += 1;
    }
  }
}

fn push_diag(stmt_start: usize, abs_s: usize, abs_e: usize, op: &str, col: &str, out: &mut Vec<Diagnostic>) {
  let negated = op.starts_with("NOT ") || op == "!~" || op == "!~*";
  let msg = if negated {
    format!(
      "`{col} {op} {col}` -- a column compared against itself is always FALSE (for non-NULL rows); almost always a copy-paste typo for two distinct columns."
    )
  } else {
    format!(
      "`{col} {op} {col}` -- a column compared against itself is always TRUE (for non-NULL rows); the predicate has no filter effect. Almost always a copy-paste typo for two distinct columns."
    )
  };
  out.push(Diagnostic {
    code: "sql508",
    severity: Severity::Warning,
    message: msg,
    range: TextRange::new(((stmt_start + abs_s) as u32).into(), ((stmt_start + abs_e) as u32).into()),
  });
}

/// Try matching `LIKE`, `ILIKE`, `NOT LIKE`, `NOT ILIKE` starting at
/// position `i`. Returns (op_text, op_start, op_end).
fn try_word_op(ub: &[u8], i: usize, pred_end: usize) -> Option<(String, usize, usize)> {
  // `LIKE` / `ILIKE` at i?
  let (op_len, op_text) = if i + 4 <= pred_end && &ub[i..i + 4] == b"LIKE" && (i + 4 == ub.len() || !is_word(ub[i + 4] as char)) && (i == 0 || !is_word(ub[i - 1] as char)) {
    (4, "LIKE".to_string())
  } else if i + 5 <= pred_end && &ub[i..i + 5] == b"ILIKE" && (i + 5 == ub.len() || !is_word(ub[i + 5] as char)) && (i == 0 || !is_word(ub[i - 1] as char)) {
    (5, "ILIKE".to_string())
  } else {
    return None;
  };
  // Check for preceding `NOT`.
  let mut prev = i;
  while prev > 0 && ub[prev - 1].is_ascii_whitespace() {
    prev -= 1;
  }
  if prev >= 3 && &ub[prev - 3..prev] == b"NOT" && (prev == 3 || !is_word(ub[prev - 4] as char)) {
    Some((format!("NOT {op_text}"), prev - 3, i + op_len))
  } else {
    Some((op_text, i, i + op_len))
  }
}

/// Try matching `~`, `~*`, `!~`, `!~*` at position `i`. Returns
/// (op_text, op_start, op_end).
fn try_symbol_op(bytes: &[u8], i: usize, pred_end: usize) -> Option<(String, usize, usize)> {
  let n = pred_end;
  if i + 3 <= n && &bytes[i..i + 3] == b"!~*" {
    return Some(("!~*".into(), i, i + 3));
  }
  if i + 2 <= n && &bytes[i..i + 2] == b"!~" {
    return Some(("!~".into(), i, i + 2));
  }
  if i + 2 <= n && &bytes[i..i + 2] == b"~*" {
    return Some(("~*".into(), i, i + 2));
  }
  // `~` alone -- but only if not preceded by `!`.
  if i < n && bytes[i] == b'~' && (i == 0 || bytes[i - 1] != b'!') {
    return Some(("~".into(), i, i + 1));
  }
  None
}

/// Read LHS (walking back from op_start) and RHS (walking forward
/// from op_end). Returns (lhs, rhs, abs_s, abs_e) where abs_s/abs_e
/// span the full `lhs op rhs` expression in raw.
fn extract_lhs_rhs<'a>(raw: &'a str, bytes: &[u8], op_start: usize, op_end: usize, pred_start: usize, pred_end: usize) -> Option<(&'a str, &'a str, usize, usize)> {
  let mut l = op_start;
  while l > pred_start && bytes[l - 1].is_ascii_whitespace() {
    l -= 1;
  }
  let lhs_end = l;
  while l > pred_start {
    let b = bytes[l - 1];
    if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
      l -= 1;
    } else {
      break;
    }
  }
  if l == lhs_end {
    return None;
  }
  let lhs = &raw[l..lhs_end];
  let mut r = op_end;
  while r < pred_end && bytes[r].is_ascii_whitespace() {
    r += 1;
  }
  let rhs_start = r;
  while r < pred_end {
    let b = bytes[r];
    if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
      r += 1;
    } else {
      break;
    }
  }
  if r == rhs_start {
    return None;
  }
  let rhs = &raw[rhs_start..r];
  Some((lhs, rhs, l, r))
}

/// Compare two identifiers ignoring qualifier (alias.col matches col
/// if the bare names match). Returns true iff the bare column names
/// are the same AND the qualifiers (if both present) match too.
fn idents_eq(a: &str, b: &str) -> bool {
  let a_bare = a.rsplit('.').next().unwrap_or(a);
  let b_bare = b.rsplit('.').next().unwrap_or(b);
  if !a_bare.eq_ignore_ascii_case(b_bare) {
    return false;
  }
  // If both sides are qualified, require the qualifiers match too
  // (so `u.id IS DISTINCT FROM v.id` doesn't fire).
  let a_q = if a.contains('.') { a.rsplit_once('.').map(|x| x.0) } else { None };
  let b_q = if b.contains('.') { b.rsplit_once('.').map(|x| x.0) } else { None };
  match (a_q, b_q) {
    (Some(qa), Some(qb)) => qa.eq_ignore_ascii_case(qb),
    _ => true,
  }
}
