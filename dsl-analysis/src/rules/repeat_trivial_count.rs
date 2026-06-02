//! sql452: `repeat(s, 0)` or `repeat(s, -3)` -- a literal zero or
//! negative count always produces the empty string. The call is a
//! constant `''` that almost always indicates a typo (the user
//! meant 1 / a real count) or a leftover placeholder.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql452"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let needle = b"REPEAT";
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
      if args.len() == 2
        && let Ok(v) = args[1].trim().parse::<i64>()
        && v <= 0
      {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        let label = if v == 0 { "0" } else { "a negative literal" };
        out.push(Diagnostic {
          code: "sql452",
          severity: Severity::Warning,
          message: format!(
            "`repeat(..., {v})` always returns the empty string (count is {label}); almost always a typo for a real count or a leftover placeholder"
          ),
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
