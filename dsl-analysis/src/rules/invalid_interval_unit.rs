//! sql440: `INTERVAL '2 mans'` -- the unit word is not a recognized
//! PG interval unit. PG raises 22007 "invalid input syntax for type
//! interval" at execution. Almost always a typo (`mans` -> `months`,
//! `weak` -> `weeks`, `yeers` -> `years`). The check only fires for
//! the `<number> <word>` shape; ISO 8601 (`P1Y2M3D`), bare
//! `HH:MM:SS`, and bare numbers are left alone.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

/// Recognized PG interval unit words. Singular + plural + common
/// abbreviations. Source: src/backend/utils/adt/datetime.c DATETKTBL.
/// Lower-cased, deduplicated. Anything not in this list inside an
/// INTERVAL literal's unit slot is almost certainly a typo.
const UNITS: &[&str] = &[
  "microsecond",
  "microseconds",
  "us",
  "millisecond",
  "milliseconds",
  "ms",
  "second",
  "seconds",
  "s",
  "sec",
  "secs",
  "minute",
  "minutes",
  "m",
  "min",
  "mins",
  "hour",
  "hours",
  "h",
  "hr",
  "hrs",
  "day",
  "days",
  "d",
  "week",
  "weeks",
  "w",
  "wk",
  "wks",
  "month",
  "months",
  "mon",
  "mons",
  "year",
  "years",
  "y",
  "yr",
  "yrs",
  "decade",
  "decades",
  "dec",
  "decs",
  "century",
  "centuries",
  "c",
  "cent",
  "millennium",
  "millenniums",
  "millennia",
  "ago",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql440"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let raw_bytes = raw.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 8 <= n {
      if &ub[i..i + 8] == b"INTERVAL"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 8 == n || !is_word(ub[i + 8] as char))
      {
        let mut j = i + 8;
        while j < raw_bytes.len() && raw_bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j >= raw_bytes.len() || raw_bytes[j] != b'\'' {
          i += 8;
          continue;
        }
        let lit_start = j + 1;
        let mut k = lit_start;
        while k < raw.len() {
          if raw_bytes[k] == b'\'' {
            if k + 1 < raw.len() && raw_bytes[k + 1] == b'\'' {
              k += 2;
              continue;
            }
            break;
          }
          k += 1;
        }
        if k >= raw.len() {
          i += 8;
          continue;
        }
        let lit = &raw[lit_start..k];
        if let Some(bad) = first_bad_unit(lit) {
          let abs_s = start + i;
          let abs_e = start + k + 1;
          out.push(Diagnostic {
            code: "sql440",
            severity: Severity::Error,
            message: format!(
              "INTERVAL literal `'{lit}'` has unrecognized unit `{bad}` -- PG raises 22007 \"invalid input syntax for type interval\"; valid units are year(s)/month(s)/week(s)/day(s)/hour(s)/minute(s)/second(s)/millisecond(s)/microsecond(s) and standard abbreviations"
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
        i = k + 1;
        continue;
      }
      i += 1;
    }
  }
}

/// Returns the first unrecognized unit word in the literal, or None
/// when the literal is empty / pure number / ISO 8601 / bare HH:MM:SS
/// (those don't follow the `<number> <word>` shape that this check
/// targets).
fn first_bad_unit(lit: &str) -> Option<String> {
  let trimmed = lit.trim();
  // ISO 8601 starts with P (case-insensitive).
  if trimmed.starts_with(['P', 'p']) {
    // Validate it looks like `P...` and skip.
    return None;
  }
  // Pure HH:MM:SS / HH:MM -- no spaces, contains colons.
  if !trimmed.contains(char::is_whitespace) {
    return None;
  }
  // Walk tokens: skip number tokens (and negative sign), check word tokens.
  for tok in trimmed.split_ascii_whitespace() {
    if is_numeric_token(tok) {
      continue;
    }
    // Pure word -- check against UNITS.
    let lower = tok.trim_end_matches(',').to_ascii_lowercase();
    if !UNITS.contains(&lower.as_str()) {
      return Some(tok.to_string());
    }
  }
  None
}

fn is_numeric_token(t: &str) -> bool {
  let t = t.trim_start_matches(['-', '+']);
  if t.is_empty() {
    return false;
  }
  // Allow integer, decimal, and time-form (`12:34:56`) inside the
  // literal -- those are valid number-ish parts of the interval.
  t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':')
}
