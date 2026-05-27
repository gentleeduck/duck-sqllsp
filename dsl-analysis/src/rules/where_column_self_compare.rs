//! sql408: `WHERE col = col` (or `<col> OP <col>` for the same column
//! on both sides). The predicate is either a tautology (`=`, `<=`,
//! `>=`) or trivially-false (`<`, `>`, `<>`, `!=`), modulo NULL. Almost
//! always a typo for `col = other_col` or `col = literal`.

use crate::clause_scan::{find_clause, find_clause_end, is_word};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql408"
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
    let bytes_u = upper.as_bytes();
    // Scan each WHERE clause (multiple SELECTs in one statement is
    // rare but possible via subselects -- text-scanner walks them
    // all because find_clause iterates the whole body).
    let stopwords = ["GROUP BY", "ORDER BY", "LIMIT", "OFFSET", "HAVING", "FOR", "FETCH", "WINDOW", "RETURNING", "UNION", "INTERSECT", "EXCEPT"];
    let mut search_from = 0usize;
    let mut emitted: std::collections::HashSet<String> = std::collections::HashSet::new();
    while let Some(rel) = find_clause_starting_at(&bytes_u[search_from..], b"WHERE").map(|p| p + search_from) {
      let pred_start = rel + 5;
      let pred_end = find_clause_end(bytes_u, pred_start, &stopwords);
      scan_predicate(&cleaned, pred_start, pred_end, start, &mut emitted, out);
      search_from = pred_end.max(rel + 5);
    }
    // ON clauses (JOIN ... ON ...) -- same shape, same diagnostic.
    // Use bare `ON` needle so find_clause's word-bound check fires
    // correctly; a space-padded needle would fight it whenever ON
    // sits immediately after an identifier (e.g. `u2 ON id = id`).
    let mut search_from = 0usize;
    while let Some(rel) = find_clause_starting_at(&bytes_u[search_from..], b"ON").map(|p| p + search_from) {
      let pred_start = rel + 2;
      let pred_end = find_clause_end(bytes_u, pred_start, &stopwords);
      scan_predicate(&cleaned, pred_start, pred_end, start, &mut emitted, out);
      search_from = pred_end.max(rel + 2);
    }
  }
}

fn find_clause_starting_at(bytes: &[u8], needle: &[u8]) -> Option<usize> {
  find_clause(bytes, needle)
}

fn scan_predicate(
  cleaned: &str,
  pred_start: usize,
  pred_end: usize,
  abs_offset: usize,
  emitted: &mut std::collections::HashSet<String>,
  out: &mut Vec<Diagnostic>,
) {
  let bytes = cleaned.as_bytes();
  let n = pred_end.min(bytes.len());
  let mut i = pred_start;
  while i < n {
    // Skip string literals.
    if bytes[i] == b'\'' {
      i += 1;
      while i < n && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(n);
      continue;
    }
    // Find an operator token.
    let (op_str, op_len) = match operator_at(bytes, i, n) {
      Some(x) => x,
      None => {
        i += 1;
        continue;
      },
    };
    // Identifier immediately before (skipping whitespace).
    let mut left_end = i;
    while left_end > pred_start && bytes[left_end - 1].is_ascii_whitespace() {
      left_end -= 1;
    }
    let left = read_ident_backward(bytes, pred_start, left_end);
    let mut right_start = i + op_len;
    while right_start < n && bytes[right_start].is_ascii_whitespace() {
      right_start += 1;
    }
    let right = read_ident_forward(bytes, right_start, n);
    if let (Some((ls, _le, lstr)), Some((_rs, re, rstr))) = (left, right) {
      // Numeric literals look like identifiers to the byte scanner
      // (`1` is is_word_byte=true). They're not columns, so `1=1`
      // is sql282's concern, not ours. Skip when either side is a
      // pure number.
      let is_numeric = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.');
      if is_numeric(&lstr) || is_numeric(&rstr) {
        i = re;
        continue;
      }
      if lstr.eq_ignore_ascii_case(&rstr) {
        let dedup = format!("{}:{}:{}", ls, op_str, rstr.to_ascii_lowercase());
        if emitted.insert(dedup) {
          let abs_s = abs_offset + ls;
          let abs_e = abs_offset + re;
          out.push(Diagnostic {
            code: "sql408",
            severity: Severity::Warning,
            message: format!("`{lstr} {op_str} {rstr}` compares a column to itself -- likely a typo (NULL rows filter out, others tautology/contradiction)"),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
        i = re;
        continue;
      }
      // Advance past the right ident so we don't re-scan it.
      i = re.max(i + op_len);
      continue;
    }
    i += op_len;
  }
}

fn operator_at(bytes: &[u8], i: usize, n: usize) -> Option<(&'static str, usize)> {
  if i >= n {
    return None;
  }
  // 2-byte first so `<=`/`>=`/`<>`/`!=` aren't mistaken for `<`/`>`/`!`.
  if i + 2 <= n {
    let pair = &bytes[i..i + 2];
    match pair {
      b"<>" => return Some(("<>", 2)),
      b"!=" => return Some(("!=", 2)),
      b"<=" => return Some(("<=", 2)),
      b">=" => return Some((">=", 2)),
      _ => {},
    }
  }
  match bytes[i] {
    b'=' => Some(("=", 1)),
    b'<' => Some(("<", 1)),
    b'>' => Some((">", 1)),
    _ => None,
  }
}

/// Read a dotted identifier ending at `end` (exclusive). Returns the
/// (start, end, text) tuple or None when the byte at end-1 isn't a
/// word char.
fn read_ident_backward(bytes: &[u8], lower_bound: usize, end: usize) -> Option<(usize, usize, String)> {
  if end <= lower_bound || !is_ident_byte(bytes[end - 1]) {
    return None;
  }
  let mut start = end;
  while start > lower_bound {
    let b = bytes[start - 1];
    if is_ident_byte(b) || b == b'.' {
      start -= 1;
    } else {
      break;
    }
  }
  let text = std::str::from_utf8(&bytes[start..end]).ok()?.to_string();
  if text.is_empty() || text.starts_with('.') || text.ends_with('.') {
    return None;
  }
  Some((start, end, text))
}

/// Read a dotted identifier starting at `start`. Returns the
/// (start, end, text) tuple or None when the byte at start isn't a
/// word char.
fn read_ident_forward(bytes: &[u8], start: usize, upper_bound: usize) -> Option<(usize, usize, String)> {
  if start >= upper_bound || !is_ident_byte(bytes[start]) {
    return None;
  }
  let mut end = start;
  while end < upper_bound {
    let b = bytes[end];
    if is_ident_byte(b) || b == b'.' {
      end += 1;
    } else {
      break;
    }
  }
  let text = std::str::from_utf8(&bytes[start..end]).ok()?.to_string();
  if text.is_empty() || text.starts_with('.') || text.ends_with('.') {
    return None;
  }
  Some((start, end, text))
}

fn is_ident_byte(b: u8) -> bool {
  is_word(b as char)
}
