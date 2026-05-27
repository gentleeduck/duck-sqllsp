//! sql436: `sum(row_number() OVER (...))` -- window function nested
//! inside an aggregate. PG raises 42P20: "window function calls
//! cannot be nested inside an aggregate function call". The fix is
//! usually to wrap the windowed query in a subquery / CTE and
//! aggregate over its output, not to combine the two in one
//! expression.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

/// Aggregate function names (lowercase). Same conservative list as
/// sql424 -- false negatives OK, false positives not.
const AGGREGATES: &[&str] = &[
  "count",
  "sum",
  "avg",
  "min",
  "max",
  "array_agg",
  "string_agg",
  "json_agg",
  "jsonb_agg",
  "json_object_agg",
  "jsonb_object_agg",
  "bool_and",
  "bool_or",
  "every",
  "bit_and",
  "bit_or",
  "stddev",
  "stddev_pop",
  "stddev_samp",
  "variance",
  "var_pop",
  "var_samp",
  "corr",
  "covar_pop",
  "covar_samp",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql436"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes = cleaned.as_bytes();
    let bytes_u = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
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
      if c.is_ascii_alphabetic() && (i == 0 || !is_word(bytes_u[i - 1] as char)) {
        let word_start = i;
        let mut k = i;
        while k < n && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
          k += 1;
        }
        let name = std::str::from_utf8(&bytes[word_start..k]).unwrap_or("").to_ascii_lowercase();
        if k < n && bytes[k] == b'(' && AGGREGATES.contains(&name.as_str()) {
          // Scan inside the matching `)` for OVER followed by `(`.
          let (close_pos, over_pos) = scan_for_over(bytes, bytes_u, k + 1, n);
          if let Some(over_at) = over_pos
            && emitted.insert(word_start)
          {
            let abs_s = start + word_start;
            let abs_e = start + close_pos.min(n);
            out.push(Diagnostic {
              code: "sql436",
              severity: Severity::Error,
              message: format!(
                "window function (`OVER (...)`) cannot be nested inside aggregate `{name}(...)` -- PG raises 42P20; move the windowed expression into a subquery / CTE and aggregate over its output"
              ),
              range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
            });
            let _ = over_at;
          }
          i = close_pos.min(n).max(k + 1);
          continue;
        }
        i = k.max(i + 1);
        continue;
      }
      i += 1;
    }
  }
}

/// Walk from `from` until the matching `)` at depth 0 relative to
/// the aggregate-call open-paren. Inside, look for `OVER` followed
/// (after whitespace) by `(`. Returns (close-paren-position+1,
/// optional OVER-position).
fn scan_for_over(bytes: &[u8], bytes_u: &[u8], from: usize, n: usize) -> (usize, Option<usize>) {
  let mut depth: i32 = 1;
  let mut i = from;
  let mut over_pos: Option<usize> = None;
  while i < n && depth > 0 {
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
      i += 1;
      continue;
    }
    if c == b')' {
      depth -= 1;
      i += 1;
      if depth == 0 {
        return (i, over_pos);
      }
      continue;
    }
    if over_pos.is_none()
      && i + 4 <= n
      && &bytes_u[i..i + 4] == b"OVER"
      && (i == 0 || !is_word(bytes_u[i - 1] as char))
      && (i + 4 == n || !is_word(bytes_u[i + 4] as char))
    {
      let mut j = i + 4;
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      if j < n && bytes[j] == b'(' {
        over_pos = Some(i);
      }
      i = j;
      continue;
    }
    i += 1;
  }
  (i, over_pos)
}
