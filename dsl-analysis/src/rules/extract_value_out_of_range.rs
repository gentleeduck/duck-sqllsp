//! sql545: `WHERE EXTRACT(MONTH FROM x) = 13` / `EXTRACT(DOW FROM x) = 7` --
//! comparing an EXTRACT (or date_part) field to a value outside that field's
//! range, so the predicate never matches. `DOW` (0-6, Sunday = 0) tripping
//! people who expect 1-7 is the classic case -- they want `ISODOW`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

/// (field, inclusive min, inclusive max).
const RANGES: &[(&str, i64, i64)] = &[
  ("month", 1, 12),
  ("day", 1, 31),
  ("hour", 0, 23),
  ("minute", 0, 59),
  ("dow", 0, 6),
  ("isodow", 1, 7),
  ("quarter", 1, 4),
  ("doy", 1, 366),
  ("week", 1, 53),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql545"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let n = bytes.len();

    for (needle, is_date_part) in [("extract(", false), ("date_part(", true)] {
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(needle) {
        let at = from + rel;
        if at > 0 && is_word(bytes[at - 1] as char) {
          from = at + needle.len();
          continue;
        }
        let open = at + needle.len() - 1;
        let Some(close) = match_paren(bytes, open) else { break };
        let field = if is_date_part {
          first_string_arg(&lower, open + 1, close)
        } else {
          first_word(&lower, open + 1, close)
        };
        from = close + 1;
        let Some(field) = field else { continue };
        let Some(&(_, lo, hi)) = RANGES.iter().find(|(f, _, _)| *f == field) else { continue };
        // `= <int>` right after the call.
        let mut p = skip_ws(bytes, close + 1);
        if bytes.get(p) != Some(&b'=') || bytes.get(p + 1) == Some(&b'=') {
          continue;
        }
        p = skip_ws(bytes, p + 1);
        let Some((val, end)) = read_int(bytes, p, n) else { continue };
        if val < lo || val > hi {
          let hint = if field == "dow" && val == 7 {
            " (DOW is 0-6, Sunday=0; use ISODOW for 1-7)"
          } else {
            ""
          };
          out.push(Diagnostic {
            code: "sql545",
            severity: Severity::Warning,
            message: format!("`{field}` ranges {lo}-{hi}, so `= {val}` never matches{hint}"),
            range: crate::range_at(start + at, start + end),
          });
        }
      }
    }
  }
}

fn first_word(lower: &str, from: usize, to: usize) -> Option<String> {
  let bytes = lower.as_bytes();
  let s = skip_ws(bytes, from);
  let mut e = s;
  while e < to && (bytes[e].is_ascii_alphanumeric() || bytes[e] == b'_') {
    e += 1;
  }
  if e == s { None } else { Some(lower[s..e].to_string()) }
}

fn first_string_arg(lower: &str, from: usize, to: usize) -> Option<String> {
  let bytes = lower.as_bytes();
  let s = skip_ws(bytes, from);
  if bytes.get(s) != Some(&b'\'') {
    return None;
  }
  let mut e = s + 1;
  while e < to && bytes[e] != b'\'' {
    e += 1;
  }
  Some(lower[s + 1..e].to_string())
}

fn read_int(bytes: &[u8], start: usize, to: usize) -> Option<(i64, usize)> {
  let mut i = start;
  if bytes.get(i) == Some(&b'-') {
    i += 1;
  }
  let ds = i;
  while i < to && bytes[i].is_ascii_digit() {
    i += 1;
  }
  if i == ds {
    return None;
  }
  // Reject `13.0` / `13x` so we only match a clean integer literal.
  if matches!(bytes.get(i), Some(&b) if b == b'.' || is_word(b as char)) {
    return None;
  }
  let v: i64 = std::str::from_utf8(&bytes[start..i]).ok()?.parse().ok()?;
  Some((v, i))
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
