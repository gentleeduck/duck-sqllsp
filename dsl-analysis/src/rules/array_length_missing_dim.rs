//! sql453: `array_length(arr)` -- missing dimension argument. PG's
//! signature is `array_length(anyarray, integer)`; the single-arg
//! form does not exist and the query raises 42883 "function
//! array_length(<type>[]) does not exist" at parse / execution.
//! Pass `1` as the dimension for the common 1D case, or use
//! `cardinality(arr)` (which takes no dim arg and returns 0 for
//! empty arrays).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql453"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let needle = b"ARRAY_LENGTH";
    let m = needle.len();
    let mut i = 0usize;
    while i + m <= n {
      if !(&ub[i..i + m] == needle && (i == 0 || !is_word(ub[i - 1] as char)) && (i + m == n || !is_word(ub[i + m] as char))) {
        i += 1;
        continue;
      }
      let mut k = i + m;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += m;
        continue;
      }
      let Some(close) = match_paren(bytes, k, n) else {
        i += m;
        continue;
      };
      let inner = &cleaned[k + 1..close];
      let args = split_top_commas(inner);
      if args.len() == 1 && !args[0].trim().is_empty() {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        out.push(Diagnostic {
          code: "sql453",
          severity: Severity::Error,
          message: "`array_length(arr)` is missing the dimension argument -- PG's signature is `array_length(arr, dim_int)` and raises 42883 \"function array_length(...) does not exist\". Pass `1` for the 1-dim length, or use `cardinality(arr)` (no dim arg, returns 0 for empty arrays)".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close + 1;
    }
  }
}

fn split_top_commas(s: &str) -> Vec<&str> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out: Vec<&str> = Vec::new();
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
    if c == b'(' || c == b'[' {
      depth += 1;
    } else if c == b')' || c == b']' {
      depth -= 1;
    } else if c == b',' && depth == 0 {
      out.push(&s[last..i]);
      last = i + 1;
    }
    i += 1;
  }
  out.push(&s[last..]);
  out
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
