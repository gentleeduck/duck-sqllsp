//! sql488: `jsonb_path_exists/query/query_array/query_first/match(col,
//! '<path>')` -- when the path is a string literal, it MUST start with
//! `$` (optionally prefixed by `strict ` or `lax `). PG raises a
//! runtime parse error otherwise (e.g. `ERROR: syntax error at end of
//! jsonpath input`).

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql488"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let raw_bytes = raw.as_bytes();
    let cleaned = crate::textutil::strip_noise_full(raw);
    let lower = cleaned.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = lb.len();
    let funcs: &[&[u8]] = &[
      b"jsonb_path_query_array",
      b"jsonb_path_query_first",
      b"jsonb_path_query",
      b"jsonb_path_exists",
      b"jsonb_path_match",
      // jsonpath variants for `json` type
      b"json_path_query_array",
      b"json_path_query_first",
      b"json_path_query",
      b"json_path_exists",
      b"json_path_match",
    ];
    let mut i = 0usize;
    while i < n {
      let mut matched_len: Option<usize> = None;
      for kw in funcs {
        if i + kw.len() <= n
          && &lb[i..i + kw.len()] == *kw
          && (i == 0 || !is_word(lb[i - 1] as char))
          && (i + kw.len() == n || !is_word(lb[i + kw.len()] as char))
        {
          matched_len = Some(kw.len());
          break;
        }
      }
      let Some(name_len) = matched_len else {
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
      // Find the first top-level comma -- the 2nd arg is the path.
      let commas = find_top_commas(raw_bytes, inner_start, inner_end);
      if !commas.is_empty() {
        let arg2_start = commas[0] + 1;
        let arg2_end = if commas.len() >= 2 { commas[1] } else { inner_end };
        let raw_arg2 = raw[arg2_start..arg2_end].trim();
        // Only check string-literal paths -- bind params / column refs
        // are runtime-unknown.
        if raw_arg2.starts_with('\'') && raw_arg2.ends_with('\'') && raw_arg2.len() >= 2 {
          let inner = &raw_arg2[1..raw_arg2.len() - 1];
          if !is_valid_jsonpath_prefix(inner) {
            let abs_s = start + i;
            let abs_e = start + close + 1;
            out.push(Diagnostic {
              code: "sql488",
              severity: Severity::Error,
              message: format!(
                "jsonpath literal `{raw_arg2}` is missing the `$` root anchor -- PG raises a syntax error at runtime. Paths must start with `$` (optionally prefixed by `strict ` or `lax `), e.g. `$.field` or `strict $.a.b`."
              ),
              range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
          }
        }
      }
      i = close + 1;
    }
  }
}

fn is_valid_jsonpath_prefix(path: &str) -> bool {
  let s = path.trim_start();
  if s.starts_with('$') {
    return true;
  }
  // Strip optional `strict ` or `lax ` prefix (case-insensitive),
  // then require `$`.
  let lc = s.to_ascii_lowercase();
  for kw in ["strict ", "lax "] {
    if let Some(rest) = lc.strip_prefix(kw) {
      let original_offset = s.len() - rest.len();
      let after = s[original_offset..].trim_start();
      if after.starts_with('$') {
        return true;
      }
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
