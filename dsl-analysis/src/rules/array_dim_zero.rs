//! sql487: `array_length(arr, 0)`, `array_lower(arr, 0)`,
//! `array_upper(arr, 0)`, or any negative dimension -- PG array
//! dimensions are 1-based; an out-of-range dimension makes the
//! function silently return NULL. Almost always a typo for `1`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql487"
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
    let names: &[&[u8]] = &[b"array_length", b"array_lower", b"array_upper"];
    let mut i = 0usize;
    while i < n {
      let mut matched: Option<usize> = None;
      for kw in names {
        if i + kw.len() <= n && &lb[i..i + kw.len()] == *kw && (i == 0 || !is_word(lb[i - 1] as char)) {
          let after = i + kw.len();
          if after == n || !is_word(lb[after] as char) {
            matched = Some(kw.len());
            break;
          }
        }
      }
      let Some(name_len) = matched else {
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
      if commas.len() == 1 {
        let raw_dim = raw[(commas[0] + 1)..inner_end].trim();
        if let Some(reason) = classify_bad_dim(raw_dim) {
          let abs_s = start + i;
          let abs_e = start + close + 1;
          out.push(Diagnostic {
            code: "sql487",
            severity: Severity::Warning,
            message: format!(
              "`array_length/lower/upper(..., {raw_dim})` -- PG array dimensions are 1-based; {reason}. The call returns NULL. Use 1 for the first dimension."
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
      }
      i = close + 1;
    }
  }
}

fn classify_bad_dim(s: &str) -> Option<&'static str> {
  let t = s.trim();
  if t == "0" {
    return Some("dimension 0 is invalid");
  }
  // Strip an optional `::int` cast for matching.
  let core = t.split("::").next().unwrap_or(t).trim();
  if core == "0" {
    return Some("dimension 0 is invalid");
  }
  if let Some(rest) = core.strip_prefix('-')
    && rest.chars().all(|c| c.is_ascii_digit())
    && !rest.is_empty()
  {
    return Some("negative dimensions are invalid");
  }
  None
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
