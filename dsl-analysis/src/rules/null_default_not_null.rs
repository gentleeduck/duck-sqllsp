//! sql069: column declared `NOT NULL` but `DEFAULT NULL`.
//!
//! `NOT NULL DEFAULT NULL` is contradictory; the very first row insert
//! that omits the column will violate the NOT NULL constraint. Error.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql069"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(_) = &stmt.kind else {
      return;
    };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Walk each `(` paren-list entry. Look for both `NOT NULL` and
    // `DEFAULT NULL` on the same entry.
    let bytes = upper.as_bytes();
    let _n = bytes.len();
    let Some(open) = upper.find('(') else { return };
    let Some(close) = match_paren(bytes, open) else { return };
    let body_text = &upper[open + 1..close];
    for (entry_off, entry) in split_top_level_commas_with_pos(body_text) {
      let has_not_null = contains_word(entry, "NOT NULL");
      // Also catch `DEFAULT CAST(NULL AS ...)` and `DEFAULT (NULL)`
      // -- both behave the same as `DEFAULT NULL`.
      let has_default_null = contains_word(entry, "DEFAULT NULL")
        || contains_substring_ci(entry, "DEFAULT CAST(NULL")
        || contains_substring_ci(entry, "DEFAULT (NULL)")
        || contains_substring_ci(entry, "DEFAULT (NULL ");
      if has_not_null && has_default_null {
        // Narrow the diagnostic to just the offending column
        // declaration -- skip leading/trailing whitespace.
        let leading = entry.len() - entry.trim_start().len();
        let trimmed = entry.trim();
        let abs_start = start + open + 1 + entry_off + leading;
        let abs_end = abs_start + trimmed.len();
        out.push(Diagnostic {
          code: "sql069",
          severity: Severity::Error,
          message: "column is NOT NULL but DEFAULT NULL -- the default would always violate the constraint".into(),
          range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
        });
        return;
      }
    }
  }
}

fn split_top_level_commas_with_pos(s: &str) -> Vec<(usize, &str)> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out = Vec::new();
  let mut start = 0usize;
  let mut depth = 0i32;
  let mut i = 0;
  while i < n {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => {
        out.push((start, &s[start..i]));
        start = i + 1;
      },
      _ => {},
    }
    i += 1;
  }
  out.push((start, &s[start..n]));
  out
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = open;
  while i < n {
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
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn contains_word(haystack: &str, needle: &str) -> bool {
  let h = haystack.as_bytes();
  let n = needle.as_bytes();
  let mut i = 0;
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

/// Case-insensitive substring search (no word-boundary requirement,
/// since the haystack is already uppercased by the caller).
fn contains_substring_ci(haystack: &str, needle: &str) -> bool {
  let h = haystack.as_bytes();
  let n = needle.as_bytes();
  if n.is_empty() || n.len() > h.len() {
    return false;
  }
  let mut i = 0usize;
  while i + n.len() <= h.len() {
    if h[i..i + n.len()].eq_ignore_ascii_case(n) {
      return true;
    }
    i += 1;
  }
  false
}
