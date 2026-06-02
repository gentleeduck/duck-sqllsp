//! sql450: `NUMERIC(p, s)` (or `DECIMAL(p, s)`) with `s > p`. PG
//! raises 22023 "NUMERIC scale N must be between 0 and precision P"
//! at parse time. The numeric type's invariant is that the scale (
//! digits after the decimal point) cannot exceed the precision (
//! total digits), so the column / cast can never store a value.
//! Likely a swapped-arg typo (the user meant `NUMERIC(s, p)`).
//! Also flags `NUMERIC(0, ...)` since precision must be positive.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

const TYPES: &[&[u8]] = &[b"NUMERIC", b"DECIMAL"];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql450"
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
    let mut i = 0usize;
    while i < n {
      let mut matched: Option<(&[u8], usize)> = None;
      for t in TYPES {
        let m = t.len();
        if i + m <= n
          && &ub[i..i + m] == *t
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + m == n || !is_word(ub[i + m] as char))
        {
          matched = Some((t, m));
          break;
        }
      }
      let Some((tname, m)) = matched else {
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
      let type_str = std::str::from_utf8(tname).unwrap_or("NUMERIC").to_string();
      let report = |sev: Severity, msg: String, out: &mut Vec<Diagnostic>| {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        out.push(Diagnostic {
          code: "sql450",
          severity: sev,
          message: msg,
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      };
      match args.len() {
        1 => {
          if let Ok(p) = args[0].trim().parse::<i64>()
            && p <= 0
          {
            report(
              Severity::Error,
              format!("{type_str}({p}) -- precision must be positive; PG raises 22023 at parse time"),
              out,
            );
          }
        },
        2 => {
          if let (Ok(p), Ok(s)) = (args[0].trim().parse::<i64>(), args[1].trim().parse::<i64>()) {
            if p <= 0 {
              report(
                Severity::Error,
                format!("{type_str}({p}, {s}) -- precision must be positive; PG raises 22023"),
                out,
              );
            } else if s > p {
              report(
                Severity::Error,
                format!(
                  "{type_str}({p}, {s}) -- scale ({s}) exceeds precision ({p}); PG raises 22023 \"NUMERIC scale {s} must be between 0 and precision {p}\". Did you swap the arguments?"
                ),
                out,
              );
            }
          }
        },
        _ => {},
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
