//! sql442: `regexp_replace(s, pattern, replacement)` -- PG's default
//! behavior is to replace only the FIRST match, not all matches. The
//! global-replace behavior (matching common-language intuition and
//! most other regex libraries) requires an explicit 4th-arg flag
//! string containing `g` (`regexp_replace(s, pattern, replacement,
//! 'g')`). Same goes for `regexp_replace(s, pat, repl, 'i')` --
//! case-insensitive but still single-replace.
//!
//! Fires on calls with 3 args (no flag arg) or 4 args where the flag
//! is a string literal NOT containing `g`. Skip when the flag arg is
//! a non-literal (variable / column) since we can't determine its
//! contents at edit time.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql442"
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
    let raw_bytes = raw.as_bytes();
    let n = ub.len();
    let needle = b"REGEXP_REPLACE";
    let m = needle.len();
    let mut i = 0usize;
    while i + m <= n {
      if &ub[i..i + m] == needle
        && (i == 0 || !is_word(ub[i - 1] as char))
      {
        // Expect `(` immediately or after whitespace.
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
        let inner = &cleaned[k + 1..close];
        let args = split_top_commas(inner);
        let nargs = args.len();
        // Map arg offsets back to raw (string-literal contents are
        // blanked in cleaned, so re-read flag arg from raw).
        let warn = match nargs {
          3 => Some("missing 4th-arg flag string -- without `'g'` PG replaces only the FIRST match (not all matches as most regex libraries default to); add `'g'` to replace all or use `replace()` for literal single-replace".to_string()),
          4 => {
            // Re-read the 4th arg from raw.
            let arg_off = args[3].1;
            let arg_abs_start = (k + 1) + arg_off;
            let arg_abs_end = arg_abs_start + args[3].0.len();
            let raw_arg = &raw[arg_abs_start..arg_abs_end.min(raw.len())];
            let trimmed = raw_arg.trim();
            if trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2 {
              let flag = &trimmed[1..trimmed.len() - 1];
              if !flag.contains('g') && !flag.contains('G') {
                Some(format!(
                  "flag string `{flag}` does not contain `g` -- PG replaces only the FIRST match; add `g` to replace all (e.g. `'{flag}g'`)"
                ))
              } else {
                None
              }
            } else {
              // Non-literal flag arg -- can't tell.
              None
            }
          },
          _ => None,
        };
        let _ = raw_bytes; // suppress unused
        if let Some(msg) = warn {
          let abs_s = start + i;
          let abs_e = start + close + 1;
          out.push(Diagnostic {
            code: "sql442",
            severity: Severity::Warning,
            message: msg,
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
        i = close + 1;
        continue;
      }
      i += 1;
    }
  }
}

/// Split `s` on top-level commas (depth 0, string-aware). Returns
/// (slice, relative-offset-in-s) pairs so callers can map back into
/// the surrounding buffer.
fn split_top_commas(s: &str) -> Vec<(&str, usize)> {
  let bytes = s.as_bytes();
  let n = bytes.len();
  let mut out: Vec<(&str, usize)> = Vec::new();
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
    if c == b'(' {
      depth += 1;
    } else if c == b')' {
      depth -= 1;
    } else if c == b',' && depth == 0 {
      out.push((&s[last..i], last));
      last = i + 1;
    }
    i += 1;
  }
  out.push((&s[last..], last));
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
