//! sql476: `CASE col WHEN NULL THEN ...` -- in PG's simple-CASE form
//! the WHEN value is compared with `=`, and `col = NULL` evaluates
//! to NULL (not TRUE), so the branch never matches. The user almost
//! certainly meant the searched form
//! `CASE WHEN col IS NULL THEN ...` instead. (Searched CASE handles
//! NULLs explicitly via IS / IS NOT operators.)

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql476"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + 4 <= n {
      if !word_eq(ub, i, b"CASE") {
        i += 1;
        continue;
      }
      // Find the END of the case expression to bound our scan.
      let (case_end, after_case_off) = match scan_case_when_nulls(ub, bytes, i + 4) {
        Some(x) => x,
        None => {
          i += 4;
          continue;
        },
      };
      // `after_case_off` is the offset (relative to case-start) of the
      // first detected `WHEN NULL`. None means no offending WHEN.
      if let Some(when_at) = after_case_off {
        // Need to confirm this is the SIMPLE form, not searched.
        // Simple form has a non-empty expression between CASE and the
        // first WHEN. Walk from i+4 to first WHEN at depth 0; if it
        // contains anything non-whitespace, it's the simple form.
        let first_when = find_first_when(ub, bytes, i + 4, case_end);
        let is_simple = if let Some(fw) = first_when {
          cleaned[(i + 4)..fw].trim().chars().any(|c| !c.is_whitespace())
        } else {
          false
        };
        if is_simple && emitted.insert(i) {
          let abs_s = start + i;
          let abs_e = start + case_end;
          let _ = when_at;
          out.push(Diagnostic {
            code: "sql476",
            severity: Severity::Warning,
            message: "`CASE <expr> WHEN NULL THEN ...` in simple-CASE form never matches -- the comparison is `<expr> = NULL` which is NULL, not TRUE. Rewrite using the searched form: `CASE WHEN <expr> IS NULL THEN ...`".into(),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
      }
      i = case_end.max(i + 4);
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

/// Walk a CASE expression from just-past `CASE`. Tracks paren depth
/// and nested CASE/END pairs. Returns (case_end_offset, optional
/// offset of an offending `WHEN NULL`).
fn scan_case_when_nulls(ub: &[u8], bytes: &[u8], from: usize) -> Option<(usize, Option<usize>)> {
  let n = ub.len();
  let mut depth_paren: i32 = 0;
  let mut depth_case: i32 = 1;
  let mut i = from;
  let mut when_null_at: Option<usize> = None;
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
          return Some((i + 3, when_null_at));
        }
        i += 3;
        continue;
      }
      if depth_case == 1 && word_eq(ub, i, b"WHEN") {
        // Read the value after WHEN until THEN at same depth.
        let val_start = i + 4;
        let then_at = find_next_then(ub, bytes, val_start, n);
        if let Some(t) = then_at {
          let raw_val = std::str::from_utf8(&bytes[val_start..t]).unwrap_or("").trim();
          if raw_val.eq_ignore_ascii_case("NULL") && when_null_at.is_none() {
            when_null_at = Some(i);
          }
          i = t;
          continue;
        }
      }
    }
    i += 1;
  }
  None
}

fn find_next_then(ub: &[u8], bytes: &[u8], from: usize, to: usize) -> Option<usize> {
  let mut depth_paren: i32 = 0;
  let mut depth_case: i32 = 0;
  let mut i = from;
  while i < to {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < to && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(to);
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
      } else if word_eq(ub, i, b"END") {
        depth_case -= 1;
      } else if depth_case == 0 && word_eq(ub, i, b"THEN") {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}

fn find_first_when(ub: &[u8], bytes: &[u8], from: usize, to: usize) -> Option<usize> {
  let mut depth_paren: i32 = 0;
  let mut depth_case: i32 = 0;
  let mut i = from;
  while i < to {
    let c = bytes[i];
    if c == b'\'' {
      i += 1;
      while i < to && bytes[i] != b'\'' {
        i += 1;
      }
      i = (i + 1).min(to);
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
      } else if word_eq(ub, i, b"END") {
        depth_case -= 1;
      } else if depth_case == 0 && word_eq(ub, i, b"WHEN") {
        return Some(i);
      }
    }
    i += 1;
  }
  None
}
