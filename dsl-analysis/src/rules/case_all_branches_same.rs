//! sql416: `CASE WHEN ... THEN x ... WHEN ... THEN x ELSE x END` --
//! every branch (including ELSE when present) returns the same value,
//! so the whole CASE expression collapses to that value. Either the
//! conditions are unintentional or the constant is.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql416"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    // Scan the *cleaned* uppercased buffer so comments and string
    // literals don't fake-match CASE/WHEN/THEN/ELSE/END. Branch text
    // is sliced from the *raw* buffer so `'x'` renders as `'x'`, not
    // the space-substituted version.
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
      // Find matching END at same nesting depth, accounting for nested
      // CASE/END pairs and parens.
      let (case_end, branches) = match scan_case(ub, bytes, raw_bytes, i + 4) {
        Some(x) => x,
        None => {
          i += 4;
          continue;
        },
      };
      if branches.len() >= 2 {
        let first = norm(&branches[0]);
        if branches.iter().all(|b| norm(b) == first) {
          let abs_s = start + i;
          let abs_e = start + case_end;
          out.push(Diagnostic {
            code: "sql416",
            severity: Severity::Hint,
            message: format!("CASE has {} branches but all return `{}` -- the expression collapses to that constant", branches.len(), first),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
      }
      i = case_end.max(i + 4);
    }
  }
}

fn scan_case(ub: &[u8], bytes: &[u8], raw_bytes: &[u8], from: usize) -> Option<(usize, Vec<String>)> {
  let n = ub.len();
  let mut depth_paren: i32 = 0;
  let mut depth_case: i32 = 1; // we're inside the outer CASE
  let mut branches: Vec<String> = Vec::new();
  let mut i = from;
  let mut current_start: Option<usize> = None;
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
      // Word-bounded scan.
      if word_eq(ub, i, b"CASE") {
        depth_case += 1;
        i += 4;
        continue;
      }
      if word_eq(ub, i, b"END") {
        depth_case -= 1;
        if depth_case == 0 {
          // Close current branch if any.
          if let Some(s) = current_start.take()
            && let Ok(text) = std::str::from_utf8(&raw_bytes[s..i])
          {
            branches.push(text.trim().to_string());
          }
          // Return the position just after END.
          return Some((i + 3, branches));
        }
        i += 3;
        continue;
      }
      if depth_case == 1 {
        if word_eq(ub, i, b"THEN") {
          // Branch value starts after THEN, ends at next WHEN/ELSE/END
          // at the same depth.
          if let Some(s) = current_start.take()
            && let Ok(text) = std::str::from_utf8(&raw_bytes[s..i])
          {
            branches.push(text.trim().to_string());
          }
          current_start = Some(i + 4);
          i += 4;
          continue;
        }
        if word_eq(ub, i, b"WHEN") {
          if let Some(s) = current_start.take()
            && let Ok(text) = std::str::from_utf8(&raw_bytes[s..i])
          {
            branches.push(text.trim().to_string());
          }
          i += 4;
          continue;
        }
        if word_eq(ub, i, b"ELSE") {
          if let Some(s) = current_start.take()
            && let Ok(text) = std::str::from_utf8(&raw_bytes[s..i])
          {
            branches.push(text.trim().to_string());
          }
          current_start = Some(i + 4);
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
  s.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase()
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
