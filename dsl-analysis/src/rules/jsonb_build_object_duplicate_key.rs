//! sql449: `jsonb_build_object('k', 1, 'k', 2)` -- duplicate key. PG
//! silently overwrites the earlier value with the later one in the
//! resulting JSON object, so the first pair is dead. Almost always
//! a copy-paste typo. Same for `json_build_object`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

const FNS: &[&[u8]] = &[b"JSONB_BUILD_OBJECT", b"JSON_BUILD_OBJECT"];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql449"
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
      let mut matched: Option<(&[u8], usize)> = None;
      for f in FNS {
        let m = f.len();
        if i + m <= n
          && &ub[i..i + m] == *f
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + m == n || !is_word(ub[i + m] as char))
        {
          matched = Some((f, m));
          break;
        }
      }
      let Some((_fname, m)) = matched else {
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
      // Split args in the cleaned buffer; but read each key arg from
      // RAW source (string literal contents are blanked in cleaned).
      let inner_start = k + 1;
      let inner_end = close;
      let inner_cleaned = &cleaned[inner_start..inner_end];
      let arg_offsets = split_top_commas_offsets(inner_cleaned);
      // Collect (key_text_lowercase, span) for every even-indexed
      // arg that's a string literal in raw source.
      let mut seen: Vec<(String, usize, usize)> = Vec::new();
      let mut dup: Option<(String, usize, usize)> = None;
      for (idx, &(arg_off, arg_end)) in arg_offsets.iter().enumerate() {
        if idx % 2 != 0 {
          continue;
        }
        let raw_arg_start = inner_start + arg_off;
        let raw_arg_end = inner_start + arg_end;
        let raw_arg = raw[raw_arg_start..raw_arg_end.min(raw.len())].trim();
        if !(raw_arg.starts_with('\'') && raw_arg.ends_with('\'') && raw_arg.len() >= 2) {
          continue;
        }
        let key = raw_arg[1..raw_arg.len() - 1].to_string();
        let key_lc = key.to_ascii_lowercase();
        if let Some((prev_key, _ps, _pe)) = seen.iter().find(|(k, _, _)| k == &key_lc) {
          dup = Some((prev_key.clone(), raw_arg_start, raw_arg_end));
          break;
        }
        seen.push((key_lc, raw_arg_start, raw_arg_end));
        let _ = raw_bytes;
      }
      if let Some((dup_key, s, e)) = dup {
        let abs_s = start + s;
        let abs_e = start + e;
        out.push(Diagnostic {
          code: "sql449",
          severity: Severity::Warning,
          message: format!(
            "duplicate key `'{dup_key}'` in jsonb_build_object -- PG silently keeps the LAST value; the earlier pair is dead. Drop the duplicate or fix the key spelling"
          ),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close + 1;
    }
  }
}

fn split_top_commas_offsets(s: &str) -> Vec<(usize, usize)> {
  // Returns (start, end) byte offsets within `s` for each top-level arg.
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
