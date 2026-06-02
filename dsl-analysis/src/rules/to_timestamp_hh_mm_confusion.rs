//! sql454: `to_timestamp(s, 'HH:MM')` -- in PG's datetime template
//! language, `MM` means MONTH (not minute). `MI` is the minute
//! token. Users coming from strftime / Java SimpleDateFormat / Python
//! / Ruby routinely write `HH:MM` thinking it means HH:minute, but
//! PG silently parses (and TO_CHAR formats) it as HH:MONTH. The
//! result is wrong values without any runtime error. Same gotcha
//! applies to `MM:SS` (where the user meant `MI:SS`).
//!
//! Covers to_timestamp, to_char, and to_date format-string literals.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

const FNS: &[&[u8]] = &[b"TO_TIMESTAMP", b"TO_CHAR", b"TO_DATE"];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql454"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let raw_bytes = raw.as_bytes();
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i < n {
      let mut matched: Option<usize> = None;
      for f in FNS {
        let m = f.len();
        if i + m <= n
          && &ub[i..i + m] == *f
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + m == n || !is_word(ub[i + m] as char))
        {
          matched = Some(m);
          break;
        }
      }
      let Some(m) = matched else {
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
      // Second arg is the format string. Split args on top-level
      // commas in cleaned, but read the 2nd arg from raw to recover
      // the literal contents.
      let inner_start = k + 1;
      let inner_end = close;
      let arg_offsets = split_top_commas_offsets(&cleaned[inner_start..inner_end]);
      if arg_offsets.len() < 2 {
        i = close + 1;
        continue;
      }
      let (fs, fe) = arg_offsets[1];
      let raw_fmt = &raw[(inner_start + fs)..(inner_start + fe).min(raw.len())].trim();
      if !(raw_fmt.starts_with('\'') && raw_fmt.ends_with('\'') && raw_fmt.len() >= 2) {
        i = close + 1;
        continue;
      }
      let fmt = &raw_fmt[1..raw_fmt.len() - 1];
      if let Some(reason) = scan_format(fmt) {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        out.push(Diagnostic {
          code: "sql454",
          severity: Severity::Warning,
          message: reason,
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      let _ = raw_bytes;
      i = close + 1;
    }
  }
}

/// Returns Some(message) when the format string contains an `HH:MM`,
/// `HH24:MM`, `HH12:MM`, or `MM:SS` pattern (MM = month; almost
/// always a minute typo).
fn scan_format(fmt: &str) -> Option<String> {
  let upper = fmt.to_ascii_uppercase();
  // We don't have regex; do a small hand-rolled scan.
  let bytes = upper.as_bytes();
  let n = bytes.len();
  let mut i = 0usize;
  while i < n {
    // Look for HH | HH24 | HH12 followed by `:` then MM.
    if i + 2 <= n && &bytes[i..i + 2] == b"HH" {
      let mut j = i + 2;
      if j + 2 <= n && (&bytes[j..j + 2] == b"24" || &bytes[j..j + 2] == b"12") {
        j += 2;
      }
      if j < n && bytes[j] == b':' && j + 3 <= n && &bytes[j + 1..j + 3] == b"MM" {
        // Word boundary after MM: end-of-string, non-letter.
        let after = j + 3;
        let bound_ok = after == n || !bytes[after].is_ascii_alphabetic();
        if bound_ok {
          return Some(format!(
            "format string `'{fmt}'` uses `MM` after `HH` -- in PG's datetime template, `MM` means MONTH (not minute). You almost certainly meant `MI` for minutes; values will be silently wrong"
          ));
        }
      }
    }
    // Look for MM:SS (month-second juxtaposition is almost always wrong).
    if i + 5 <= n && &bytes[i..i + 5] == b"MM:SS" {
      let prev_ok = i == 0 || !bytes[i - 1].is_ascii_alphabetic();
      let after = i + 5;
      let bound_ok = after == n || !bytes[after].is_ascii_alphabetic();
      if prev_ok && bound_ok {
        return Some(format!(
          "format string `'{fmt}'` uses `MM:SS` -- in PG's datetime template, `MM` means MONTH (not minute). You almost certainly meant `MI:SS` for minutes/seconds; values will be silently wrong"
        ));
      }
    }
    i += 1;
  }
  None
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
