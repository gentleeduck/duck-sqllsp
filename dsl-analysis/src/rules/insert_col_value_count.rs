//! sql038: `INSERT INTO t (a, b) VALUES (1)` -- column-list length must
//! match the VALUES tuple length.
//!
//! Postgres raises `INSERT has more/fewer expressions than target
//! columns`. We catch at edit time via direct text scan since the
//! parser exposes only the column list, not the VALUES count.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql038"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(i) = &stmt.kind else {
      return;
    };
    if i.columns.is_empty() {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();

    // Only check INSERT ... VALUES (...) form -- not INSERT ... SELECT.
    // If the body contains SELECT at depth 0 between the column list
    // and the would-be VALUES, the INSERT uses a SELECT source and the
    // VALUES we see is inside a derived table (`FROM (VALUES ...)`).
    let bytes = body.as_bytes();
    let n = bytes.len();
    let values_kw = upper.find("VALUES");
    let Some(values_at) = values_kw else {
      return;
    };
    let prefix_upper = &upper[..values_at];
    if contains_word_depth0(prefix_upper, "SELECT") {
      return;
    }
    let mut k = values_at + 6;
    while k < n && bytes[k].is_ascii_whitespace() {
      k += 1;
    }
    if k >= n || bytes[k] != b'(' {
      return;
    }
    let Some(tuple_end) = match_paren(bytes, k) else {
      return;
    };
    let tuple = &body[k + 1..tuple_end];
    let value_count = top_level_comma_count(tuple) + 1;
    let col_count = i.columns.len();
    if value_count != col_count {
      // Narrow the diagnostic to the VALUES tuple `(...)` span rather
      // than the full Insert statement.range, which the parser can
      // extend past the prior `;` and land on the previous statement
      // (a CREATE INDEX one line up, etc).
      let abs_open = start + k;
      let abs_close = start + tuple_end + 1; // include the closing `)`
      out.push(Diagnostic {
        code: "sql038",
        severity: Severity::Error,
        message: format!("INSERT has {col_count} target column(s) but {value_count} value(s) in VALUES"),
        range: text_size::TextRange::new((abs_open as u32).into(), (abs_close as u32).into()),
      });
    }
  }
}

fn contains_word_depth0(haystack_upper: &str, needle: &str) -> bool {
  let h = haystack_upper.as_bytes();
  let nlen = needle.len();
  let n = h.len();
  let mut depth = 0i32;
  let mut i = 0;
  while i + nlen <= n {
    match h[i] {
      b'(' => {
        depth += 1;
        i += 1;
        continue;
      },
      b')' => {
        depth -= 1;
        i += 1;
        continue;
      },
      b'\'' => {
        i += 1;
        while i < n && h[i] != b'\'' {
          i += 1
        }
        if i < n {
          i += 1
        }
        continue;
      },
      _ => {},
    }
    if depth == 0 && haystack_upper[i..i + nlen].eq_ignore_ascii_case(needle) {
      let prev_ok = i == 0 || !(h[i - 1].is_ascii_alphanumeric() || h[i - 1] == b'_');
      let next_ok = i + nlen == n || !(h[i + nlen].is_ascii_alphanumeric() || h[i + nlen] == b'_');
      if prev_ok && next_ok {
        return true;
      }
    }
    i += 1;
  }
  false
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let n = bytes.len();
  let mut depth = 0i32;
  let mut bracket_depth = 0i32;
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
      b'[' => bracket_depth += 1,
      b']' => bracket_depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    let _ = bracket_depth;
    i += 1;
  }
  None
}

fn top_level_comma_count(s: &str) -> usize {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut count = 0usize;
  let mut depth = 0i32;
  let mut i = 0;
  while i < n {
    match bytes[i] {
      // Track `[` / `]` too so commas inside `ARRAY['a','b']` or
      // `col[1]` don't count as top-level value separators.
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => count += 1,
      _ => {},
    }
    i += 1;
  }
  count
}
