//! sql484: `OVER (PARTITION BY <constant> ...)` -- partitioning by
//! a constant expression collapses every row into a single window,
//! which is equivalent to having no PARTITION BY at all. Counterpart
//! to sql480 (GROUP BY constant) and sql433 (ORDER BY constant) for
//! the window-function PARTITION BY clause.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql484"
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
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + 12 <= n {
      if !(word_eq_multi(ub, i, b"PARTITION", b"BY")) {
        i += 1;
        continue;
      }
      // Advance past "PARTITION BY" (handle the run of whitespace
      // between the two words).
      let mut k = i + 9; // past "PARTITION"
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      // Should be "BY" here.
      if k + 2 > n || &ub[k..k + 2] != b"BY" {
        i += 9;
        continue;
      }
      k += 2;
      // Collect items: walk forward tracking paren depth, stop on
      // ORDER BY at the same depth, on RANGE/ROWS/GROUPS frame
      // keywords, or on the closing paren that brings us below the
      // starting depth.
      let scan_start = k;
      let mut depth: i32 = 0;
      let mut j = k;
      let mut stop = n;
      while j < n {
        let c = bytes[j];
        if c == b'\'' {
          j += 1;
          while j < n && bytes[j] != b'\'' {
            j += 1;
          }
          j = (j + 1).min(n);
          continue;
        }
        if c == b'(' {
          depth += 1;
          j += 1;
          continue;
        }
        if c == b')' {
          if depth == 0 {
            stop = j;
            break;
          }
          depth -= 1;
          j += 1;
          continue;
        }
        if depth == 0 {
          // ORDER BY / RANGE / ROWS / GROUPS terminate PARTITION BY.
          if word_eq_multi(ub, j, b"ORDER", b"BY") {
            stop = j;
            break;
          }
          if word_eq(ub, j, b"RANGE") || word_eq(ub, j, b"ROWS") || word_eq(ub, j, b"GROUPS") {
            stop = j;
            break;
          }
        }
        j += 1;
      }
      // Split items in [scan_start..stop] on top-level commas.
      let items = split_top_commas(&cleaned[scan_start..stop], &raw[scan_start..stop]);
      for (raw_item, item_off) in items {
        let trimmed = raw_item.trim();
        if classify_constant(trimmed).is_none() {
          continue;
        }
        let abs_item_s = start + scan_start + item_off + (raw_item.len() - raw_item.trim_start().len());
        let abs_item_e = abs_item_s + trimmed.len();
        if emitted.insert(abs_item_s) {
          out.push(Diagnostic {
            code: "sql484",
            severity: Severity::Warning,
            message: "`PARTITION BY <constant>` collapses every row into a single window -- equivalent to having no PARTITION BY at all. Drop the clause or partition by a real column.".into(),
            range: TextRange::new((abs_item_s as u32).into(), (abs_item_e as u32).into()),
          });
        }
      }
      i = stop.max(i + 1);
    }
  }
}

fn word_eq(ub: &[u8], i: usize, w: &[u8]) -> bool {
  let m = w.len();
  if i + m > ub.len() {
    return false;
  }
  if &ub[i..i + m] != w {
    return false;
  }
  let prev_ok = i == 0 || !is_word(ub[i - 1] as char);
  let next_ok = i + m == ub.len() || !is_word(ub[i + m] as char);
  prev_ok && next_ok
}

fn word_eq_multi(ub: &[u8], i: usize, w1: &[u8], w2: &[u8]) -> bool {
  if !word_eq(ub, i, w1) {
    return false;
  }
  let mut k = i + w1.len();
  while k < ub.len() && (ub[k] as char).is_whitespace() {
    k += 1;
  }
  word_eq(ub, k, w2)
}

fn classify_constant(s: &str) -> Option<&'static str> {
  let t = s.trim();
  if t.is_empty() {
    return None;
  }
  let u = t.to_ascii_uppercase();
  if u == "NULL" {
    return Some("NULL");
  }
  if u == "TRUE" || u == "FALSE" {
    return Some("bool");
  }
  if t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2 {
    return Some("string");
  }
  // Bare integer / decimal literal: every char is digit / dot / leading sign
  let mut chars = t.chars();
  let first = chars.next()?;
  if first == '-' || first == '+' {
    // continue
  } else if !first.is_ascii_digit() && first != '.' {
    return None;
  }
  if t.chars().filter(|c| !c.is_ascii_digit() && *c != '.' && *c != '+' && *c != '-').count() == 0 {
    return Some("number");
  }
  None
}

fn split_top_commas<'a>(cleaned: &'a str, raw: &'a str) -> Vec<(&'a str, usize)> {
  let mut out = Vec::new();
  let bytes = cleaned.as_bytes();
  let n = bytes.len();
  let mut depth: i32 = 0;
  let mut start_off = 0usize;
  let mut i = 0usize;
  while i < n {
    let c = bytes[i];
    if c == b'(' || c == b'[' {
      depth += 1;
    } else if c == b')' || c == b']' {
      depth -= 1;
    } else if c == b',' && depth == 0 {
      out.push((&raw[start_off..i], start_off));
      start_off = i + 1;
    }
    i += 1;
  }
  out.push((&raw[start_off..n], start_off));
  out
}
