//! sql483: `split_part(<s>, <delim>, 0)` -- PG raises a runtime
//! error: `field position must not be zero` (since the n argument
//! is 1-indexed, with negative values counting from the end in
//! PG 14+). The literal `0` will always blow up at execution time.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql483"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
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
      if !(i + 10 <= n && &ub[i..i + 10] == b"SPLIT_PART" && (i == 0 || !is_word(ub[i - 1] as char))) {
        i += 1;
        continue;
      }
      let mut k = i + 10;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += 10;
        continue;
      }
      let open = k;
      let Some(close) = match_paren(bytes, open, n) else {
        i += 10;
        continue;
      };
      let inner_start = open + 1;
      let inner_end = close;
      // Find 2 top-level commas to isolate the 3rd argument.
      let commas = find_top_commas(raw_bytes, inner_start, inner_end);
      if commas.len() == 2 {
        let arg3 = raw[(commas[1] + 1)..inner_end].trim();
        if is_literal_zero(arg3) {
          let abs_s = start + i;
          let abs_e = start + close + 1;
          out.push(Diagnostic {
            code: "sql483",
            severity: Severity::Error,
            message: "`split_part(..., ..., 0)` -- PG raises `field position must not be zero` (the n argument is 1-indexed; negative values count from the end in PG 14+). Use 1 (first field), -1 (last field), or another non-zero position.".into(),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
          });
        }
      }
      i = close + 1;
    }
  }
}

fn is_literal_zero(s: &str) -> bool {
  let s = s.trim();
  if s == "0" {
    return true;
  }
  if let Some(rest) = s.strip_prefix("0::") {
    return rest.chars().all(is_word);
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
