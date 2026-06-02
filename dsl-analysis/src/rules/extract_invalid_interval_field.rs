//! sql472: `EXTRACT(dow FROM '1 day'::interval)` -- `dow`, `doy`,
//! `week`, `isodow`, `isoyear`, `julian`, `timezone*` are not
//! valid fields for an INTERVAL operand. PG raises 22023
//! "unit X not supported for type interval" at execution.
//!
//! Only fires when the EXTRACT's FROM expression is recognizably an
//! interval literal (`INTERVAL '...'` keyword form or `'...'::interval`
//! cast form). Other types (date / timestamp / time) take a wider
//! valid-field set and are not checked here.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

const INVALID_INTERVAL_FIELDS: &[&str] = &[
  "dow",
  "doy",
  "week",
  "isodow",
  "isoyear",
  "julian",
  "timezone",
  "timezone_hour",
  "timezone_minute",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql472"
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
    let needle = b"EXTRACT";
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
      let inner_upper = inner.to_ascii_uppercase();
      // Find ` FROM ` separator (case-insensitive, word-bounded).
      let Some(from_at) = find_word_at(inner_upper.as_bytes(), b"FROM") else {
        i = close + 1;
        continue;
      };
      let field = inner[..from_at].trim();
      let operand_upper = &inner_upper[from_at + 4..];
      // Heuristic: operand recognizable as interval literal.
      let is_interval = operand_upper.contains("INTERVAL ") || operand_upper.contains("::INTERVAL") || operand_upper.contains("::PG_CATALOG.INTERVAL");
      if !is_interval {
        i = close + 1;
        continue;
      }
      let flo = field.to_ascii_lowercase();
      if INVALID_INTERVAL_FIELDS.iter().any(|f| *f == flo) {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        out.push(Diagnostic {
          code: "sql472",
          severity: Severity::Error,
          message: format!(
            "EXTRACT field `{flo}` is not valid for an INTERVAL operand -- PG raises 22023 \"unit {flo} not supported for type interval\" at execution. Valid interval fields: year/month/day/hour/minute/second/decade/century/millennium/microseconds/milliseconds/epoch"
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close + 1;
    }
  }
}

fn find_word_at(bytes: &[u8], w: &[u8]) -> Option<usize> {
  let m = w.len();
  let n = bytes.len();
  let mut i = 0usize;
  while i + m <= n {
    if &bytes[i..i + m] == w
      && (i == 0 || !is_word(bytes[i - 1] as char))
      && (i + m == n || !is_word(bytes[i + m] as char))
    {
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
