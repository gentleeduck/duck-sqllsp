//! sql087: `x BETWEEN <high> AND <low>` -- bounds are flipped and the
//! expression matches nothing. Catches numeric literal cases
//! (constant low > constant high) and string-literal cases
//! (lex-order swap, which is correct for TEXT columns and ISO-format
//! date/timestamp literals).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql087"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let raw_bytes = body.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 7 <= n {
      if &upper[i..i + 7] == "BETWEEN" {
        let prev_ok = i == 0 || !is_word(bytes[i - 1] as char);
        let next_ok = i + 7 == n || !is_word(bytes[i + 7] as char);
        if prev_ok && next_ok {
          let mut j = i + 7;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          let Some((low_kind, low_text, low_end)) = read_bound(raw_bytes, bytes, j, n) else {
            i += 1;
            continue;
          };
          j = low_end;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          if j + 3 > n || &upper[j..j + 3] != "AND" {
            i += 1;
            continue;
          }
          j += 3;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          let Some((high_kind, high_text, high_end)) = read_bound(raw_bytes, bytes, j, n) else {
            i += 1;
            continue;
          };
          j = high_end;
          if low_kind != high_kind {
            i += 1;
            continue;
          }
          let swapped = match low_kind {
            BoundKind::Numeric => match (low_text.parse::<f64>(), high_text.parse::<f64>()) {
              (Ok(lo), Ok(hi)) => lo > hi,
              _ => false,
            },
            // For string literals, strip the surrounding quotes and
            // compare lex order. This is correct for TEXT columns and
            // ISO-format date/timestamp literals (where lex order
            // matches semantic order).
            BoundKind::String => {
              let lo = strip_quotes(&low_text);
              let hi = strip_quotes(&high_text);
              lo > hi
            },
          };
          if swapped {
            let (is_not, kw_start) = scan_preceding_not(bytes, i);
            let abs_start = start + kw_start;
            let abs_end = start + j;
            let (severity, message) = if is_not {
              (
                Severity::Warning,
                format!("NOT BETWEEN {low_text} AND {high_text}: low > high makes the inner BETWEEN always false, so NOT BETWEEN matches every row -- almost certainly a swapped-bound typo"),
              )
            } else {
              (
                Severity::Error,
                match low_kind {
                  BoundKind::Numeric => format!("BETWEEN {low_text} AND {high_text}: low > high, the expression matches no rows"),
                  BoundKind::String => format!("BETWEEN {low_text} AND {high_text}: low > high in lex order (correct for TEXT columns and ISO-format date/timestamp literals); the expression matches no rows -- swap the bounds"),
                },
              )
            };
            out.push(Diagnostic {
              code: "sql087",
              severity,
              message,
              range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
            });
            return;
          }
        }
      }
      i += 1;
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundKind {
  Numeric,
  String,
}

/// Reads a numeric (digits/./- prefix) or string-literal ('...') bound
/// starting at `from`. Returns (kind, text, end_offset).
fn read_bound(raw: &[u8], _upper: &[u8], from: usize, n: usize) -> Option<(BoundKind, String, usize)> {
  if from >= n {
    return None;
  }
  // String literal
  if raw[from] == b'\'' {
    let mut j = from + 1;
    while j < n {
      if raw[j] == b'\'' {
        // `''` escape inside the literal
        if j + 1 < n && raw[j + 1] == b'\'' {
          j += 2;
          continue;
        }
        let text = std::str::from_utf8(&raw[from..j + 1]).ok()?.to_string();
        return Some((BoundKind::String, text, j + 1));
      }
      j += 1;
    }
    return None;
  }
  // Numeric (optionally signed)
  let mut j = from;
  if raw[j] == b'-' || raw[j] == b'+' {
    j += 1;
  }
  let num_start = j;
  while j < n && (raw[j].is_ascii_digit() || raw[j] == b'.') {
    j += 1;
  }
  if j > num_start {
    let text = std::str::from_utf8(&raw[from..j]).ok()?.to_string();
    return Some((BoundKind::Numeric, text, j));
  }
  None
}

fn strip_quotes(s: &str) -> &str {
  let s = s.trim();
  if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
    &s[1..s.len() - 1]
  } else {
    s
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

/// Walk backwards from `between_pos` over whitespace; if the previous
/// word is `NOT`, return (true, start-of-NOT) so the diagnostic range
/// can cover the NOT too.
fn scan_preceding_not(bytes: &[u8], between_pos: usize) -> (bool, usize) {
  let mut j = between_pos;
  while j > 0 && bytes[j - 1].is_ascii_whitespace() {
    j -= 1;
  }
  // Read backwards to find the previous word.
  let word_end = j;
  while j > 0 && is_word(bytes[j - 1] as char) {
    j -= 1;
  }
  if word_end - j == 3 && bytes[j..word_end].eq_ignore_ascii_case(b"NOT") {
    return (true, j);
  }
  (false, between_pos)
}
