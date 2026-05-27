//! sql467: `replace(s, '', x)` / `split_part(s, '', n)` -- the needle
//! / delimiter argument is the empty string. PG semantics:
//! - `replace(s, '', x)` returns `s` unchanged (the empty needle is
//!   never "found").
//! - `split_part(s, '', n)` returns `s` for n=1, '' otherwise.
//!
//! Both are effectively no-ops and almost always a leftover
//! placeholder.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

/// (function name, which-arg-is-the-needle, message-tag).
/// 1 means the 2nd argument (zero-indexed: args[1]).
const FNS: &[(&[u8], usize, &str)] = &[
  (b"REPLACE", 1, "replace"),
  (b"SPLIT_PART", 1, "split_part"),
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql467"
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
    let n = ub.len();
    let mut i = 0usize;
    while i < n {
      let mut matched: Option<(&[u8], usize, usize, &str)> = None;
      for (name, idx, tag) in FNS {
        let m = name.len();
        if i + m <= n
          && &ub[i..i + m] == *name
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + m == n || !is_word(ub[i + m] as char))
        {
          matched = Some((name, m, *idx, tag));
          break;
        }
      }
      let Some((_name, m, needle_idx, tag)) = matched else {
        i += 1;
        continue;
      };
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
      // Split args from cleaned but read each from RAW since
      // literal contents are blanked in cleaned.
      let inner_start = k + 1;
      let inner_end = close;
      let arg_offsets = split_top_commas_offsets(&cleaned[inner_start..inner_end]);
      if let Some(&(fs, fe)) = arg_offsets.get(needle_idx) {
        let abs_arg_start = inner_start + fs;
        let abs_arg_end = inner_start + fe;
        let raw_arg = raw[abs_arg_start..abs_arg_end.min(raw.len())].trim();
        if raw_arg == "''" {
          let abs_s = start + i;
          let abs_e = start + close + 1;
          out.push(Diagnostic {
            code: "sql467",
            severity: Severity::Warning,
            message: format!(
              "`{tag}(..., '', ...)` has an empty needle/delimiter -- PG returns the input unchanged (or empty for split_part n>1); the call is effectively a no-op"
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
      }
      i = close + 1;
    }
  }
}

fn split_top_commas_offsets(s: &str) -> Vec<(usize, usize)> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out: Vec<(usize, usize)> = Vec::new();
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
      out.push((last, i));
      last = i + 1;
    }
    i += 1;
  }
  out.push((last, n));
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
