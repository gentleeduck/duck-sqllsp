//! sql447: `power(x, 0)` always returns 1 and `power(x, 1)` always
//! returns x. Both are tautologies that almost always indicate a
//! typo, a leftover placeholder, or a misunderstanding of which
//! arg is the base vs the exponent (PG signature is
//! `power(base, exponent)`).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql447"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
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
    let needle = b"POWER";
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
        let args = split_top_commas(inner);
        if args.len() == 2 {
          let exp = args[1].0.trim();
          if let Some(reason) = trivial_exponent(exp, args[0].0.trim()) {
            let abs_s = start + i;
            let abs_e = start + close + 1;
            out.push(Diagnostic {
              code: "sql447",
              severity: Severity::Hint,
              message: reason,
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

fn trivial_exponent(exp: &str, base: &str) -> Option<String> {
  match exp.parse::<f64>() {
    Ok(0.0) => Some(format!(
      "`power({base}, 0)` always returns 1 -- the exponent is 0, so the call is a constant; was the base/exponent order swapped?"
    )),
    Ok(1.0) => Some(format!(
      "`power({base}, 1)` always returns `{base}` -- the exponent is 1, so the call is a no-op; was the base/exponent order swapped?"
    )),
    _ => None,
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
