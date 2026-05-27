//! sql451: `VARCHAR(0)` / `CHAR(0)` / `CHARACTER(0)` / `CHARACTER
//! VARYING(0)` -- a zero-length string type. PG accepts the
//! declaration, but the column can only ever store the empty string
//! (any non-empty input raises 22001 "value too long"). Almost
//! always a typo (the user meant `VARCHAR(10)` etc.) or a
//! placeholder left in a refactor.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

/// Recognized character / bit-string type keyword sequences. Order
/// matters: longer forms first so `CHARACTER VARYING` wins over bare
/// `CHARACTER` and `BIT VARYING` wins over bare `BIT`.
const TYPES: &[&str] = &["CHARACTER VARYING", "BIT VARYING", "CHARACTER", "VARCHAR", "CHAR", "BIT"];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql451"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i < n {
      let mut matched: Option<(&'static str, usize)> = None;
      for kw in TYPES {
        let kb = kw.as_bytes();
        let m = kb.len();
        if i + m <= n
          && &ub[i..i + m] == kb
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + m == n || !is_word(ub[i + m] as char))
        {
          matched = Some((*kw, m));
          break;
        }
      }
      let Some((kw, m)) = matched else {
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
      let inner = cleaned[k + 1..close].trim();
      if let Ok(n_chars) = inner.parse::<i64>()
        && n_chars == 0
      {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        let is_bit = kw.contains("BIT");
        let message = if is_bit {
          format!(
            "{kw}(0) declares a zero-length bit string column -- it can only hold the empty bit string; any non-empty input raises a length error. Almost certainly a typo (did you mean {kw}(8) / {kw}(32) / ...?)"
          )
        } else {
          format!(
            "{kw}(0) declares a zero-length string column -- it can only hold the empty string; any non-empty input raises 22001 \"value too long\". Almost certainly a typo (did you mean {kw}(10) / {kw}(255) / TEXT?)"
          )
        };
        out.push(Diagnostic {
          code: "sql451",
          severity: Severity::Warning,
          message,
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close + 1;
    }
  }
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
