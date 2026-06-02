//! sql444: `generate_series(1, 10, 0)` -- a literal zero step is a
//! runtime error in PG (22023 "step size cannot equal zero"). Also
//! covers a step whose sign points the wrong way for the start/end
//! range (e.g. `generate_series(10, 1, 1)` produces an empty set
//! because the step moves the cursor further from the end). Fires
//! only when args are integer literals.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql444"
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
    let needle = b"GENERATE_SERIES";
    let m = needle.len();
    let mut i = 0usize;
    while i + m <= n {
      if !(&ub[i..i + m] == needle && (i == 0 || !is_word(ub[i - 1] as char))) {
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
      if let Some((sev, msg)) = classify(&args) {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        out.push(Diagnostic { code: "sql444", severity: sev, message: msg, range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()) });
      }
      i = close + 1;
    }
  }
}

fn classify(args: &[(&str, usize)]) -> Option<(Severity, String)> {
  match args.len() {
    2 => {
      let a = parse_signed_int(args[0].0.trim())?;
      let b = parse_signed_int(args[1].0.trim())?;
      if a > b {
        Some((
          Severity::Warning,
          format!(
            "generate_series({a}, {b}): start > end with no explicit step -- PG defaults step to 1, so this produces an EMPTY set; pass `-1` as the third arg for a descending range"
          ),
        ))
      } else {
        None
      }
    },
    3 => {
      let step = parse_signed_int(args[2].0.trim())?;
      if step == 0 {
        return Some((
          Severity::Error,
          "generate_series(..., ..., 0): zero step -- PG raises 22023 \"step size cannot equal zero\" at runtime".into(),
        ));
      }
      let a = parse_signed_int(args[0].0.trim())?;
      let b = parse_signed_int(args[1].0.trim())?;
      if a > b && step > 0 {
        return Some((
          Severity::Warning,
          format!(
            "generate_series({a}, {b}, {step}): start > end with positive step -- this produces an EMPTY set; the step should be negative for a descending range"
          ),
        ));
      }
      if a < b && step < 0 {
        return Some((
          Severity::Warning,
          format!(
            "generate_series({a}, {b}, {step}): start < end with negative step -- this produces an EMPTY set; the step should be positive for an ascending range"
          ),
        ));
      }
      None
    },
    _ => None,
  }
}

fn parse_signed_int(s: &str) -> Option<i64> {
  s.trim().parse::<i64>().ok()
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
