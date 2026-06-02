//! sql446: `position('' in s)` / `strpos(s, '')` -- searching for the
//! empty string. PG returns 1 for every non-NULL `s` (the empty
//! string is found at position 1). The expression is a constant 1
//! and almost certainly a leftover placeholder where the user meant
//! to fill in the actual substring.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql446"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let raw_bytes = raw.as_bytes();
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i < n {
      // position(needle IN haystack)
      if i + 8 <= n && &ub[i..i + 8] == b"POSITION"
        && (i == 0 || !is_word(ub[i - 1] as char))
      {
        let mut k = i + 8;
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1;
        }
        if k < n && bytes[k] == b'('
          && let Some(close) = match_paren(bytes, k, n)
        {
          // PG `position(<needle> IN <haystack>)` -- split on
          // word-bounded ` IN `.
          let inner_start = k + 1;
          let inner_end = close;
          if let Some(in_at) = find_word(ub, b"IN", inner_start, inner_end) {
            let needle_abs_start = inner_start;
            let needle_abs_end = in_at;
            let raw_needle = &raw[needle_abs_start..needle_abs_end.min(raw.len())].trim();
            if is_empty_string_literal(raw_needle) {
              let abs_s = start + i;
              let abs_e = start + close + 1;
              out.push(Diagnostic {
                code: "sql446",
                severity: Severity::Warning,
                message: "`position('' in ...)` always returns 1 -- the empty string is found at every position; almost certainly a placeholder where the real substring should go".into(),
                range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
              });
            }
          }
          i = close + 1;
          continue;
        }
        i += 8;
        continue;
      }
      // strpos(haystack, needle)
      if i + 6 <= n && &ub[i..i + 6] == b"STRPOS"
        && (i == 0 || !is_word(ub[i - 1] as char))
      {
        let mut k = i + 6;
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1;
        }
        if k < n && bytes[k] == b'('
          && let Some(close) = match_paren(bytes, k, n)
        {
          let inner_start = k + 1;
          let inner_end = close;
          // Find top-level comma in raw source.
          if let Some(comma) = find_top_comma(raw_bytes, inner_start, inner_end) {
            let raw_needle = raw[(comma + 1)..inner_end].trim();
            if is_empty_string_literal(raw_needle) {
              let abs_s = start + i;
              let abs_e = start + close + 1;
              out.push(Diagnostic {
                code: "sql446",
                severity: Severity::Warning,
                message: "`strpos(..., '')` always returns 1 -- the empty string is found at every position; almost certainly a placeholder where the real substring should go".into(),
                range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
              });
            }
          }
          i = close + 1;
          continue;
        }
        i += 6;
        continue;
      }
      i += 1;
    }
  }
}

fn is_empty_string_literal(s: &str) -> bool {
  let t = s.trim();
  t == "''"
}

fn find_word(ub: &[u8], w: &[u8], from: usize, to: usize) -> Option<usize> {
  let m = w.len();
  let mut i = from;
  while i + m <= to {
    if &ub[i..i + m] == w
      && (i == 0 || !is_word(ub[i - 1] as char))
      && (i + m == ub.len() || !is_word(ub[i + m] as char))
    {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn find_top_comma(bytes: &[u8], from: usize, to: usize) -> Option<usize> {
  let mut depth: i32 = 0;
  let mut i = from;
  while i < to {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < to && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(to);
      continue;
    }
    if c == b'(' || c == b'[' {
      depth += 1;
    } else if c == b')' || c == b']' {
      depth -= 1;
    } else if c == b',' && depth == 0 {
      return Some(i);
    }
    i += 1;
  }
  None
}

fn match_paren(bytes: &[u8], open: usize, end: usize) -> Option<usize> {
  let mut depth: i32 = 0;
  let mut i = open;
  while i < end {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < end && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(end);
      continue;
    }
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
      if depth == 0 {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
