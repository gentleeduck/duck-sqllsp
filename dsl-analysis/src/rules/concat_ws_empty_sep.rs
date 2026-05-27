//! sql465: `concat_ws('', a, b, c)` -- empty separator is the same
//! as calling `concat(a, b, c)`. Both functions skip NULL arguments,
//! so the only role of `concat_ws`'s first arg is the separator;
//! when it's empty there's no separator and the call is identical
//! to plain `concat`. Use `concat` for clarity.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql465"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let raw_bytes = raw.as_bytes();
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let needle = b"CONCAT_WS";
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
      // First arg is the separator. Read it from RAW source since
      // strip_noise_full blanks literal contents in cleaned.
      let inner_start = k + 1;
      let inner_end = close;
      // Find the first top-level comma in cleaned, then take the
      // span [inner_start..comma] from RAW.
      let first_comma = find_top_comma(bytes, inner_start, inner_end);
      let sep_end = first_comma.unwrap_or(inner_end);
      let raw_sep = raw[inner_start..sep_end.min(raw.len())].trim();
      if raw_sep == "''" {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        out.push(Diagnostic {
          code: "sql465",
          severity: Severity::Hint,
          message: "`concat_ws('', ...)` with an empty separator is identical to `concat(...)` -- the separator never joins anything. Use `concat(...)` for clarity".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      let _ = raw_bytes;
      i = close + 1;
    }
  }
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
