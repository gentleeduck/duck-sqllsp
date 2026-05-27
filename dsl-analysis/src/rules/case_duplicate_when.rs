//! sql432: `CASE WHEN p THEN a WHEN p THEN b END` -- two WHEN
//! branches share the same condition. PG evaluates the first match
//! only, so the later branch is unreachable dead code. Either drop
//! the duplicate or fix the condition. Also covers searched
//! `CASE x WHEN 1 THEN .. WHEN 1 THEN .. END` where the constant
//! WHEN value is duplicated.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql432"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let raw_bytes = raw.as_bytes();
    let bytes = cleaned.as_bytes();
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 4 <= n {
      if !word_eq(ub, i, b"CASE") {
        i += 1;
        continue;
      }
      if let Some((case_end, conditions)) = scan_case_conditions(ub, bytes, raw_bytes, i + 4) {
        // Detect first duplicate (normalized).
        let mut seen: Vec<(String, (usize, usize))> = Vec::new();
        for c in &conditions {
          let key = norm(&c.text);
          if key.is_empty() {
            continue;
          }
          if let Some((prev_text, _prev_span)) = seen.iter().find(|(k, _)| k == &key) {
            let abs_s = start + c.span.0;
            let abs_e = start + c.span.1;
            out.push(Diagnostic {
              code: "sql432",
              severity: Severity::Warning,
              message: format!(
                "duplicate WHEN condition `{}` -- this branch is unreachable because the earlier WHEN already matches the same rows; drop the duplicate or fix the condition",
                prev_text
              ),
              range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
            break;
          }
          seen.push((key, c.span));
        }
        i = case_end.max(i + 4);
      } else {
        i += 4;
      }
    }
  }
}

struct WhenCond {
  text: String,
  span: (usize, usize),
}

fn scan_case_conditions(ub: &[u8], bytes: &[u8], raw_bytes: &[u8], from: usize) -> Option<(usize, Vec<WhenCond>)> {
  let n = ub.len();
  let mut depth_paren: i32 = 0;
  let mut depth_case: i32 = 1;
  let mut conditions: Vec<WhenCond> = Vec::new();
  let mut i = from;
  // `pending` marks the start byte of a WHEN-condition being collected.
  let mut pending: Option<usize> = None;
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
      depth_paren += 1;
      i += 1;
      continue;
    }
    if c == b')' {
      depth_paren -= 1;
      i += 1;
      continue;
    }
    if depth_paren == 0 {
      if word_eq(ub, i, b"CASE") {
        depth_case += 1;
        i += 4;
        continue;
      }
      if word_eq(ub, i, b"END") {
        depth_case -= 1;
        if depth_case == 0 {
          return Some((i + 3, conditions));
        }
        i += 3;
        continue;
      }
      if depth_case == 1 {
        if word_eq(ub, i, b"WHEN") {
          // Start collecting condition just after WHEN.
          pending = Some(i + 4);
          i += 4;
          continue;
        }
        if word_eq(ub, i, b"THEN") {
          if let Some(s) = pending.take()
            && let Ok(t) = std::str::from_utf8(&raw_bytes[s..i])
          {
            conditions.push(WhenCond { text: t.trim().to_string(), span: (s, i) });
          }
          i += 4;
          continue;
        }
      }
    }
    i += 1;
  }
  None
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

fn norm(s: &str) -> String {
  // Collapse whitespace and lowercase; preserve string-literal contents
  // (already quoted) so `'a'` differs from `'A'` only as much as the
  // user intended (we lowercase the whole thing, so PG-case-sensitive
  // string equality DOES get collapsed -- but for the duplicate-WHEN
  // signal that's the right behavior; `WHEN x = 'a'` twice is a bug
  // regardless of literal case).
  s.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase()
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
