//! sql473: `col = ANY(ARRAY[]::int[])` -- empty array on the
//! RHS of an ANY-comparison. PG returns FALSE (no row matches);
//! the predicate filters out everything. The `ALL` variant returns
//! TRUE (vacuously true) which is also almost always a bug.
//!
//! Also covers the bare empty-array literal `'{}'::<type>[]`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql473"
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
    while i + 3 <= n {
      // Match ANY( or ALL(
      let kw = if i + 3 <= n && &ub[i..i + 3] == b"ANY" && (i == 0 || !is_word(ub[i - 1] as char)) {
        Some("ANY")
      } else if i + 3 <= n && &ub[i..i + 3] == b"ALL" && (i == 0 || !is_word(ub[i - 1] as char)) {
        Some("ALL")
      } else {
        None
      };
      let Some(kw) = kw else {
        i += 1;
        continue;
      };
      let mut k = i + 3;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += 3;
        continue;
      }
      let Some(close) = match_paren(bytes, k, n) else {
        i += 3;
        continue;
      };
      let inner_abs_start = k + 1;
      let inner_abs_end = close;
      let raw_inner = raw[inner_abs_start..inner_abs_end.min(raw.len())].trim();
      // Detect empty array forms:
      //   ARRAY[] or ARRAY[]::<type>[]
      //   '{}' or '{}'::<type>[]
      let raw_upper = raw_inner.to_ascii_uppercase();
      let is_empty_array =
        raw_upper.starts_with("ARRAY[]") || raw_inner.starts_with("'{}'") || raw_inner.starts_with("'{}'::");
      if is_empty_array {
        let outcome = if kw == "ANY" { "always FALSE -- no row can match an empty set" } else { "always TRUE (vacuously) -- every row satisfies the empty universal" };
        let abs_s = start + i;
        let abs_e = start + close + 1;
        let _ = raw_bytes;
        out.push(Diagnostic {
          code: "sql473",
          severity: Severity::Warning,
          message: format!(
            "`= {kw}(<empty-array>)` is {outcome}; the comparison degenerates to a constant. Did you mean to filter on a non-empty list?"
          ),
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
