//! sql439: `DATE '2024-13-01'` / `TIMESTAMP '2024-02-30'` -- typed
//! date/time literals with an out-of-range month or day. PG raises
//! 22008 "date/time field value out of range" at parse / execution
//! time depending on the path. Catches obvious calendar mistakes
//! (month > 12, day > 31, day > days-in-month). Leap year is
//! respected (Feb 29 valid only in leap years).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

/// (keyword, validate-date?, validate-time?)
const KEYWORDS: &[(&str, bool, bool)] = &[
  ("DATE", true, false),
  ("TIMESTAMP", true, true),
  ("TIMESTAMPTZ", true, true),
  ("TIME", false, true),
  ("TIMETZ", false, true),
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql439"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let n = ub.len();
    let raw_bytes = raw.as_bytes();
    let mut i = 0usize;
    while i < n {
      // Find a typed-literal keyword.
      let mut matched: Option<(&'static str, usize, bool, bool)> = None;
      // Iterate longest-first so TIMESTAMPTZ wins over TIMESTAMP and
      // TIMETZ wins over TIME at the same position.
      let mut ordered: Vec<&(&str, bool, bool)> = KEYWORDS.iter().collect();
      ordered.sort_by_key(|(k, _, _)| std::cmp::Reverse(k.len()));
      for (kw, vd, vt) in ordered {
        let m = kw.len();
        if i + m <= n
          && &ub[i..i + m] == kw.as_bytes()
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + m == n || !is_word(ub[i + m] as char))
        {
          matched = Some((kw, m, *vd, *vt));
          break;
        }
      }
      let Some((kw, kw_len, validate_date, validate_time)) = matched else {
        i += 1;
        continue;
      };
      // Skip whitespace, expect `'` in the raw source. Must check
      // RAW bytes for whitespace too -- strip_noise_full blanks the
      // quote AND the literal contents to spaces, so iterating on
      // `ub` would skip past the entire literal.
      let mut j = i + kw_len;
      while j < raw_bytes.len() && raw_bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j >= raw_bytes.len() || raw_bytes[j] != b'\'' {
        i += kw_len;
        continue;
      }
      // Find the closing quote in RAW source. Handles doubled-quote
      // escape `''`.
      let lit_start = j + 1;
      let mut k = lit_start;
      while k < raw.len() {
        if raw_bytes[k] == b'\'' {
          // Doubled-quote escape?
          if k + 1 < raw.len() && raw_bytes[k + 1] == b'\'' {
            k += 2;
            continue;
          }
          break;
        }
        k += 1;
      }
      if k >= raw.len() {
        i += kw_len;
        continue;
      }
      let lit = &raw[lit_start..k];
      let reason = if validate_date {
        validate_date_prefix(lit).or_else(|| {
          if validate_time {
            time_portion_of(lit).and_then(validate_time_hms)
          } else {
            None
          }
        })
      } else if validate_time {
        validate_time_hms(lit)
      } else {
        None
      };
      if let Some(reason) = reason {
        let abs_s = start + i;
        let abs_e = start + k + 1;
        out.push(Diagnostic {
          code: "sql439",
          severity: Severity::Error,
          message: format!(
            "{kw} literal `'{lit}'` is invalid -- {reason}; PG raises 22008 \"date/time field value out of range\""
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = k + 1;
    }
  }
}

/// Validates the leading `YYYY-MM-DD` portion of a date/timestamp
/// literal. Returns Some(reason) when the calendar is wrong, None
/// otherwise (including when the literal doesn't follow the
/// YYYY-MM-DD shape -- we don't want to be over-strict).
fn validate_date_prefix(lit: &str) -> Option<String> {
  let b = lit.as_bytes();
  if b.len() < 10 {
    return None;
  }
  // Shape: 4 digits, '-', 2 digits, '-', 2 digits.
  for (idx, expect) in [(4, b'-'), (7, b'-')] {
    if b[idx] != expect {
      return None;
    }
  }
  if !(0..4).all(|i| b[i].is_ascii_digit())
    || !(5..7).all(|i| b[i].is_ascii_digit())
    || !(8..10).all(|i| b[i].is_ascii_digit())
  {
    return None;
  }
  let y: u32 = std::str::from_utf8(&b[0..4]).ok()?.parse().ok()?;
  let m: u32 = std::str::from_utf8(&b[5..7]).ok()?.parse().ok()?;
  let d: u32 = std::str::from_utf8(&b[8..10]).ok()?.parse().ok()?;
  if !(1..=12).contains(&m) {
    return Some(format!("month {m} is out of range (1-12)"));
  }
  let dim = days_in_month(y, m);
  if !(1..=dim).contains(&d) {
    return Some(format!("day {d} is out of range for {y:04}-{m:02} (1-{dim})"));
  }
  None
}

/// Extract the time-portion of a TIMESTAMP literal: the substring
/// after the date prefix's `T` or space separator. Returns None when
/// there is no time portion (date-only timestamp).
fn time_portion_of(lit: &str) -> Option<&str> {
  if lit.len() < 11 {
    return None;
  }
  let sep = lit.as_bytes()[10];
  if sep != b' ' && sep != b'T' {
    return None;
  }
  Some(&lit[11..])
}

/// Validate an `HH:MM[:SS]` time prefix. Returns Some(reason) when
/// hour/minute/second is out of range. Ignores trailing timezone /
/// fractional-second info (PG-validated separately).
fn validate_time_hms(s: &str) -> Option<String> {
  let b = s.as_bytes();
  if b.len() < 5 {
    return None;
  }
  if !(b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b':' && b[3].is_ascii_digit() && b[4].is_ascii_digit()) {
    return None;
  }
  let h: u32 = std::str::from_utf8(&b[0..2]).ok()?.parse().ok()?;
  let m: u32 = std::str::from_utf8(&b[3..5]).ok()?.parse().ok()?;
  // PG accepts up to 24:00:00 as end-of-day; cap at 24.
  if h > 24 {
    return Some(format!("hour {h} is out of range (0-24)"));
  }
  if m > 59 {
    return Some(format!("minute {m} is out of range (0-59)"));
  }
  if b.len() >= 8 && b[5] == b':' && b[6].is_ascii_digit() && b[7].is_ascii_digit() {
    let sec: u32 = std::str::from_utf8(&b[6..8]).ok()?.parse().ok()?;
    if sec > 59 {
      return Some(format!("second {sec} is out of range (0-59)"));
    }
    if h == 24 && (m != 0 || sec != 0) {
      return Some("hour 24 only allowed as 24:00:00 (end-of-day)".into());
    }
  } else if h == 24 && m != 0 {
    return Some("hour 24 only allowed as 24:00:00 (end-of-day)".into());
  }
  None
}

fn is_leap(y: u32) -> bool {
  (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn days_in_month(y: u32, m: u32) -> u32 {
  match m {
    1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
    4 | 6 | 9 | 11 => 30,
    2 => {
      if is_leap(y) {
        29
      } else {
        28
      }
    },
    _ => 0,
  }
}
