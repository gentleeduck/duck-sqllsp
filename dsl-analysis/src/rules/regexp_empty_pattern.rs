//! sql485: `regexp_split_to_array(s, '')`, `regexp_split_to_table(s, '')`,
//! `regexp_match(s, '')`, `regexp_matches(s, '')` -- an empty regex
//! pattern matches at every position, so:
//!   * split functions return an array/set of single characters
//!   * match functions return `{""}` (an array containing one empty string)
//!
//! Almost always a placeholder bug; the user meant a real pattern.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql485"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
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
    let funcs: &[(&[u8], &str)] = &[
      (b"regexp_split_to_array", "split"),
      (b"regexp_split_to_table", "split"),
      (b"regexp_matches", "match"),
      (b"regexp_match", "match"),
    ];
    let mut i = 0usize;
    while i < n {
      let mut matched_label: Option<&str> = None;
      let mut name_len = 0usize;
      for (kw, label) in funcs {
        if i + kw.len() <= n && &lb[i..i + kw.len()] == *kw && (i == 0 || !is_word(lb[i - 1] as char)) {
          // Word-boundary on the right too -- otherwise regexp_match
          // would match regexp_matches as a prefix.
          let after = i + kw.len();
          if after == n || !is_word(lb[after] as char) {
            matched_label = Some(label);
            name_len = kw.len();
            break;
          }
        }
      }
      let Some(label) = matched_label else {
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
      // 2nd positional arg is the pattern.
      let commas = find_top_commas(raw_bytes, inner_start, inner_end);
      if commas.is_empty() {
        i = close + 1;
        continue;
      }
      let arg2_start = commas[0] + 1;
      let arg2_end = if commas.len() >= 2 { commas[1] } else { inner_end };
      let raw_arg2 = raw[arg2_start..arg2_end].trim();
      if raw_arg2 == "''" {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        let msg = match label {
          "split" => "`regexp_split_to_array/table(..., '')` -- empty pattern splits between every character; the result is the input broken into single chars. Almost certainly a placeholder where the real pattern should go.",
          _ => "`regexp_match/matches(..., '')` -- empty pattern matches at every position; the result is `{\"\"}` (an array containing one empty string). Almost certainly a placeholder where the real pattern should go.",
        };
        out.push(Diagnostic {
          code: "sql485",
          severity: Severity::Warning,
          message: msg.into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close + 1;
    }
  }
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
