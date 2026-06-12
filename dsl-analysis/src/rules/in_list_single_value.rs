//! sql515: `WHERE col IN (1)` / `WHERE col NOT IN (1)` -- an IN list with a
//! single element. Equivalent to `col = 1` / `col <> 1` but longer and a hair
//! slower to read; almost always a leftover from a list that was templated
//! down to one value. Suggests the direct comparison.
//!
//! Skips genuine multi-element lists, subqueries (`IN (SELECT ...)`),
//! `VALUES` lists, empty lists (sql234 owns those), and `IN (NULL)` -- the
//! last would rewrite to `= NULL`, which is its own (wrong) thing.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql515"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let bytes = body.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find(" IN") {
      let in_start = from + rel + 1; // index of 'I'
      let after_in = in_start + 2; // first char past "IN"
      from = after_in; // advance regardless of outcome
      // Word boundary after IN so INTO / INTERSECT / INTERVAL don't match.
      if bytes.get(after_in).is_some_and(|&b| is_word(b as char)) {
        continue;
      }
      // The next non-space char must open the list.
      let mut p = after_in;
      while p < body.len() && bytes[p].is_ascii_whitespace() {
        p += 1;
      }
      if bytes.get(p) != Some(&b'(') {
        continue;
      }
      let open = p;
      let Some(close) = find_matching_paren(body, open) else { break };
      let inner = body[open + 1..close].trim();
      let inner_upper = inner.to_ascii_uppercase();
      // Not a single scalar value -> leave it alone.
      if inner.is_empty()
        || inner.starts_with('(')
        || inner_upper.starts_with("SELECT")
        || inner_upper.starts_with("VALUES")
        || inner_upper == "NULL"
        || has_top_level_comma(inner)
      {
        continue;
      }
      let is_not = preceded_by_not(bytes, in_start);
      let op = if is_not { "<>" } else { "=" };
      let kw = if is_not { "NOT IN" } else { "IN" };
      out.push(Diagnostic {
        code: "sql515",
        severity: Severity::Hint,
        message: format!("`{kw} ({inner})` has a single element -- use `{op} {inner}` instead"),
        range: crate::range_at(start + in_start, start + close + 1),
      });
      from = close + 1;
    }
  }
}

/// True when the word immediately before `in_start` (skipping one run of
/// whitespace) is `NOT`, i.e. the construct is `NOT IN`.
fn preceded_by_not(bytes: &[u8], in_start: usize) -> bool {
  let mut j = in_start;
  while j > 0 && bytes[j - 1].is_ascii_whitespace() {
    j -= 1;
  }
  let end = j;
  while j > 0 && is_word(bytes[j - 1] as char) {
    j -= 1;
  }
  std::str::from_utf8(&bytes[j..end]).is_ok_and(|w| w.eq_ignore_ascii_case("NOT"))
}

/// True if `s` contains a comma at paren depth 0 outside single-quoted runs.
fn has_top_level_comma(s: &str) -> bool {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => return true,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  false
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
