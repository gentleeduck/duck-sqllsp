//! sql434: `WHERE col IS NOT NULL AND col = 5` -- the `IS NOT NULL`
//! check is redundant because `col = 5` (or any strict comparison /
//! IN / LIKE / BETWEEN / regex) already requires `col` to be NOT NULL
//! (PG returns NULL, not FALSE, for `NULL = anything`, and rows where
//! the WHERE predicate evaluates to NULL are discarded). The
//! redundant check costs no rows but adds noise and confuses readers
//! about whether NULLs were ever expected.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql434"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes_u = upper.as_bytes();
    let stopwords = ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    let Some(rel_where) = find_clause(bytes_u, b"WHERE") else {
      return;
    };
    let pred_start = rel_where + 5;
    let pred_end = find_clause_end(bytes_u, pred_start, &stopwords);
    let pred = &cleaned[pred_start..pred_end];
    let conjuncts = split_top_level_and(pred);
    if conjuncts.len() < 2 {
      return;
    }

    // First pass: find `<col> IS NOT NULL` conjuncts (record their
    // column identifier, normalized).
    let mut is_not_null_cols: Vec<(String, usize)> = Vec::new(); // (col_lower, conjunct_idx)
    let mut strict_cols: Vec<String> = Vec::new();
    for (idx, c) in conjuncts.iter().enumerate() {
      if let Some(col) = match_is_not_null(c) {
        is_not_null_cols.push((col.to_ascii_lowercase(), idx));
        continue;
      }
      if let Some(col) = match_strict_predicate(c) {
        strict_cols.push(col.to_ascii_lowercase());
      }
    }
    if is_not_null_cols.is_empty() || strict_cols.is_empty() {
      return;
    }
    for (col, _idx) in &is_not_null_cols {
      if strict_cols.iter().any(|s| s == col) {
        let abs_s = start + rel_where;
        let abs_e = start + pred_end;
        out.push(Diagnostic {
          code: "sql434",
          severity: Severity::Hint,
          message: format!(
            "`{col} IS NOT NULL` is redundant -- another conjunct compares `{col}` with a strict operator (=, <, >, IN, LIKE, BETWEEN, ...), which already filters out NULLs; drop the IS NOT NULL"
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
        return;
      }
    }
  }
}

/// If `c` looks like `<col> IS NOT NULL`, return the column text
/// (possibly qualified). The trailing comment-stripped clause text
/// uses spaces in place of literals, but identifiers are preserved.
fn match_is_not_null(c: &str) -> Option<String> {
  let t = c.trim();
  // Lowercase comparison for the IS NOT NULL tail, but keep the
  // column slice with its original casing.
  let tu = t.to_ascii_uppercase();
  let suffix = " IS NOT NULL";
  if !tu.ends_with(suffix) {
    return None;
  }
  let col_end = t.len() - suffix.len();
  let col = t[..col_end].trim();
  if col.is_empty() || !looks_like_column_ref(col) {
    return None;
  }
  Some(col.to_string())
}

/// If `c` is a strict (NULL-rejecting) predicate on a single column
/// in the LHS position, return that column text. Recognizes: `=`,
/// `<>`, `!=`, `<`, `>`, `<=`, `>=`, `IN (...)`, `LIKE`, `ILIKE`,
/// `~`, `~*`, `BETWEEN`.
fn match_strict_predicate(c: &str) -> Option<String> {
  let t = c.trim();
  let tu = t.to_ascii_uppercase();
  // Try operator forms first.
  for op in ["<=", ">=", "<>", "!=", "=", "<", ">"] {
    if let Some(pos) = find_top_level_op(t, op) {
      let lhs = t[..pos].trim();
      if looks_like_column_ref(lhs) {
        return Some(lhs.to_string());
      }
    }
  }
  // Word ops: <lhs> IN ( ... ), LIKE, ILIKE, BETWEEN, ~ / ~* (regex).
  // Use a sentinel-space pad so trailing-trimmed conjuncts still
  // match (`email LIKE` after literal-stripping + trim has no
  // trailing space).
  let padded = format!(" {tu} ");
  for word in [" IN ", " LIKE ", " ILIKE ", " BETWEEN ", " SIMILAR TO ", " ~* ", " ~ "] {
    if let Some(pos) = padded.find(word) {
      // Map padded-position back to original `t` (skip the leading
      // space we added).
      let orig_pos = pos.saturating_sub(1);
      let lhs = t[..orig_pos.min(t.len())].trim();
      if looks_like_column_ref(lhs) {
        return Some(lhs.to_string());
      }
    }
  }
  None
}

/// Find the byte index of `op` at top level (depth 0, no string).
fn find_top_level_op(t: &str, op: &str) -> Option<usize> {
  let bytes = t.as_bytes();
  let op_b = op.as_bytes();
  let n = bytes.len();
  let m = op_b.len();
  let mut depth: i32 = 0;
  let mut i = 0;
  while i + m <= n {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if c == b'(' {
      depth += 1;
      i += 1;
      continue;
    }
    if c == b')' {
      depth -= 1;
      i += 1;
      continue;
    }
    if depth == 0 && &bytes[i..i + m] == op_b {
      // For single-char ops, ensure they aren't part of a longer op
      // (e.g. `<=` should not match `<`).
      if op == "<" && i + 1 < n && (bytes[i + 1] == b'=' || bytes[i + 1] == b'>') {
        i += 1;
        continue;
      }
      if op == ">" && i + 1 < n && bytes[i + 1] == b'=' {
        i += 1;
        continue;
      }
      if op == "=" {
        // Avoid matching `==`, `!=`, `<=`, `>=` from the `=` side.
        if i > 0 && (bytes[i - 1] == b'!' || bytes[i - 1] == b'<' || bytes[i - 1] == b'>' || bytes[i - 1] == b'=') {
          i += 1;
          continue;
        }
      }
      return Some(i);
    }
    i += 1;
  }
  None
}

/// Heuristic: a column ref is one or two identifier-shaped tokens
/// joined by `.`, no whitespace inside. Bare keywords like NULL
/// don't qualify (they have nothing to compare).
fn looks_like_column_ref(s: &str) -> bool {
  let s = s.trim();
  if s.is_empty() {
    return false;
  }
  if !s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') {
    return false;
  }
  // Reject pure numerics.
  if s.chars().all(|c| c.is_ascii_digit() || c == '.') {
    return false;
  }
  // Reject keyword literals.
  let u = s.to_ascii_uppercase();
  if matches!(u.as_str(), "NULL" | "TRUE" | "FALSE") {
    return false;
  }
  true
}

fn split_top_level_and(s: &str) -> Vec<String> {
  let bytes = s.as_bytes();
  let upper: String = s.to_ascii_uppercase();
  let ub = upper.as_bytes();
  let n = bytes.len();
  let mut out: Vec<String> = Vec::new();
  let mut last = 0usize;
  let mut depth: i32 = 0;
  let mut i = 0;
  while i < n {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    if c == b'(' {
      depth += 1;
      i += 1;
      continue;
    }
    if c == b')' {
      depth -= 1;
      i += 1;
      continue;
    }
    if depth == 0
      && i + 3 <= n
      && &ub[i..i + 3] == b"AND"
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + 3 == n || !is_word(ub[i + 3] as char))
    {
      out.push(s[last..i].to_string());
      last = i + 3;
      i += 3;
      continue;
    }
    i += 1;
  }
  out.push(s[last..].to_string());
  out
}
