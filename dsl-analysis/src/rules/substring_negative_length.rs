//! sql443: `substring(s, start, -3)` -- a negative literal length
//! argument. PG raises 22011 "negative substring length not allowed"
//! at runtime. Almost always a typo (the user inverted the sign or
//! confused start/length). When the length arg is a non-literal
//! column / variable, we can't tell, so we stay silent.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql443"
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
    let needle = b"SUBSTRING";
    let m = needle.len();
    let mut i = 0usize;
    while i + m <= n {
      if &ub[i..i + m] == needle && (i == 0 || !is_word(ub[i - 1] as char)) {
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
        // SQL-standard form: `SUBSTRING(s FROM N FOR -k)`. Detect
        // FOR token + signed-int literal at the cleaned inner text.
        let inner_upper = inner.to_ascii_uppercase();
        if let Some(neg) = scan_for_negative(&inner_upper, inner) {
          let abs_s = start + i;
          let abs_e = start + close + 1;
          out.push(Diagnostic {
            code: "sql443",
            severity: Severity::Error,
            message: format!(
              "substring() length argument is {neg} (negative, SQL-standard `FROM ... FOR -k` form) -- PG raises 22011 \"negative substring length not allowed\" at runtime; check the sign"
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
          i = close + 1;
          continue;
        }
        let args = split_top_commas(inner);
        if args.len() == 3 {
          let len_arg = args[2].0.trim();
          if let Some(n) = parse_signed_int(len_arg)
            && n < 0
          {
            let abs_s = start + i;
            let abs_e = start + close + 1;
            out.push(Diagnostic {
              code: "sql443",
              severity: Severity::Error,
              message: format!(
                "substring() length argument is {n} (negative) -- PG raises 22011 \"negative substring length not allowed\" at runtime; check the sign or swap start/length"
              ),
              range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
          }
        }
        i = close + 1;
        continue;
      }
      i += 1;
    }
  }
}

/// Find ` FOR <signed-int>` in the inner-args text. Returns the
/// parsed negative integer when present.
fn scan_for_negative(upper: &str, orig: &str) -> Option<i64> {
  let bytes = upper.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i + 3 <= n {
    if &bytes[i..i + 3] == b"FOR"
      && (i == 0 || !is_word(bytes[i - 1] as char))
      && (i + 3 == n || !is_word(bytes[i + 3] as char))
    {
      let mut j = i + 3;
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      // Read signed int (optional minus, digits).
      let lit_start = j;
      if j < n && (bytes[j] == b'-' || bytes[j] == b'+') {
        j += 1;
      }
      let digits_start = j;
      while j < n && bytes[j].is_ascii_digit() {
        j += 1;
      }
      if j == digits_start {
        i += 1;
        continue;
      }
      let lit = &orig[lit_start..j];
      if let Ok(v) = lit.parse::<i64>()
        && v < 0
      {
        return Some(v);
      }
      i = j;
      continue;
    }
    i += 1;
  }
  None
}

fn parse_signed_int(s: &str) -> Option<i64> {
  let s = s.trim();
  if s.is_empty() {
    return None;
  }
  s.parse::<i64>().ok()
}

fn split_top_commas(s: &str) -> Vec<(&str, usize)> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out: Vec<(&str, usize)> = Vec::new();
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
    } else if c == b')' {
      depth -= 1;
    } else if c == b',' && depth == 0 {
      out.push((&s[last..i], last));
      last = i + 1;
    }
    i += 1;
  }
  out.push((&s[last..], last));
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
