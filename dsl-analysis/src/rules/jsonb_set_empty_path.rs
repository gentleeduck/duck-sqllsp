//! sql494: `jsonb_set(target, '{}', value)` / `jsonb_set_lax` /
//! `jsonb_insert` with an empty path array -- PG walks the path
//! into the target and replaces/inserts at the leaf. An empty path
//! has no leaf to update, so the call returns the original target
//! unchanged. Almost always a placeholder where the real path
//! should go.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql494"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let raw_bytes = raw.as_bytes();
    let cleaned = crate::textutil::strip_noise_full(raw);
    let lower = cleaned.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = lb.len();
    let funcs: &[&[u8]] = &[b"jsonb_set_lax", b"jsonb_insert", b"jsonb_set"];
    let mut i = 0usize;
    while i < n {
      let mut name_len: Option<usize> = None;
      for kw in funcs {
        if i + kw.len() <= n
          && &lb[i..i + kw.len()] == *kw
          && (i == 0 || !is_word(lb[i - 1] as char))
          && (i + kw.len() == n || !is_word(lb[i + kw.len()] as char))
        {
          name_len = Some(kw.len());
          break;
        }
      }
      let Some(name_len) = name_len else {
        i += 1;
        continue;
      };
      let mut k = i + name_len;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += name_len;
        continue;
      }
      let Some(close) = match_paren(bytes, k, n) else {
        i += name_len;
        continue;
      };
      let inner_start = k + 1;
      let inner_end = close;
      let commas = find_top_commas(raw_bytes, inner_start, inner_end);
      // jsonb_set has 3-4 args; jsonb_insert has 3-4 args. Path is
      // the 2nd arg.
      if !commas.is_empty() {
        let p_start = commas[0] + 1;
        let p_end = if commas.len() >= 2 { commas[1] } else { inner_end };
        let raw_path = raw[p_start..p_end].trim();
        if is_empty_path(raw_path) {
          let abs_s = start + i;
          let abs_e = start + close + 1;
          out.push(Diagnostic {
            code: "sql494",
            severity: Severity::Warning,
            message: "`jsonb_set/jsonb_set_lax/jsonb_insert(..., <empty path>, ...)` -- an empty path is a no-op (the call returns the original target unchanged). Almost certainly a placeholder where the real key path should go.".into(),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
      }
      i = close + 1;
    }
  }
}

/// True iff `s` is one of the empty-path literal forms PG accepts:
/// `'{}'`, `'{}'::text[]`, or `ARRAY[]::<type>[]`.
fn is_empty_path(s: &str) -> bool {
  let t = s.trim();
  if t == "'{}'" {
    return true;
  }
  if let Some(after) = t.strip_prefix("'{}'") {
    // Optional `::text[]` (or any array-type) cast
    let after = after.trim_start();
    if let Some(rest) = after.strip_prefix("::") {
      return rest.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '[' || c == ']');
    }
  }
  // ARRAY[]::<type>[] -- case-insensitive
  let u = t.to_ascii_uppercase();
  if let Some(after) = u.strip_prefix("ARRAY[]") {
    let after = after.trim_start();
    if after.is_empty() {
      return true;
    }
    if let Some(rest) = after.strip_prefix("::") {
      return rest.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '[' || c == ']');
    }
  }
  false
}

fn find_top_commas(bytes: &[u8], from: usize, to: usize) -> Vec<usize> {
  let mut depth: i32 = 0;
  let mut out = Vec::new();
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
    if c == b'(' || c == b'[' {
      depth += 1;
    } else if c == b')' || c == b']' {
      depth -= 1;
    } else if c == b',' && depth == 0 {
      out.push(i);
    }
    i += 1;
  }
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
