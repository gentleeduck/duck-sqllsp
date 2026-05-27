//! sql448: `lpad('hi', -3, '0')` -- negative literal length. PG
//! returns an empty string for negative `length` (truncates from the
//! right by `-length` chars; with a negative greater than the input
//! length the result is empty). Almost always a sign-flip typo. Same
//! for `rpad`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

const FNS: &[&[u8]] = &[b"LPAD", b"RPAD"];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql448"
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
      let mut matched: Option<(&[u8], usize)> = None;
      for f in FNS {
        let m = f.len();
        if i + m <= n
          && &ub[i..i + m] == *f
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + m == n || !is_word(ub[i + m] as char))
        {
          matched = Some((f, m));
          break;
        }
      }
      let Some((fname, m)) = matched else {
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
      let inner = &cleaned[k + 1..close];
      let args = split_top_commas(inner);
      if args.len() >= 2 {
        let len_arg = args[1].0.trim();
        if let Ok(v) = len_arg.parse::<i64>()
          && v < 0
        {
          let fn_str = std::str::from_utf8(fname).unwrap_or("lpad").to_ascii_lowercase();
          let abs_s = start + i;
          let abs_e = start + close + 1;
          out.push(Diagnostic {
            code: "sql448",
            severity: Severity::Warning,
            message: format!(
              "`{fn_str}(..., {v}, ...)` -- negative length pads to nothing and may truncate; PG returns an empty string for `|length| >= |input|`. Almost always a sign-flip typo"
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
      }
      i = close + 1;
    }
  }
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
    if c == b'(' || c == b'[' {
      depth += 1;
    } else if c == b')' || c == b']' {
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
