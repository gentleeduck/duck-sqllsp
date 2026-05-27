//! sql506: `ARRAY[NULL]` / `ARRAY[NULL, NULL, ...]` -- when every
//! element of the array constructor is the bare NULL keyword, PG
//! cannot determine the element type and may fall back to `text[]`
//! or raise `cannot determine type of empty array`. The result type
//! depends on context (or session config) and is rarely what the
//! author intended. Cast either an element (`NULL::int`) or the
//! whole array (`ARRAY[NULL]::int[]`) to fix the type.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql506"
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
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    while i + 5 <= n {
      if !(&ub[i..i + 5] == b"ARRAY" && (i == 0 || !is_word(ub[i - 1] as char))) {
        i += 1;
        continue;
      }
      // Expect `[` next (possibly with whitespace).
      let mut k = i + 5;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'[' {
        i += 5;
        continue;
      }
      let open = k;
      // Find matching `]`.
      let mut depth: i32 = 0;
      let mut p = open;
      while p < n {
        match bytes[p] {
          b'[' => depth += 1,
          b']' => {
            depth -= 1;
            if depth == 0 {
              break;
            }
          },
          b'\'' => {
            p += 1;
            while p < n && bytes[p] != b'\'' {
              p += 1;
            }
          },
          _ => {},
        }
        p += 1;
      }
      if p >= n {
        i += 5;
        continue;
      }
      let close = p;
      let inner = cleaned[open + 1..close].trim();
      // Skip if any element is non-NULL or has a cast.
      let mut all_null_no_cast = !inner.is_empty();
      for elem in inner.split(',') {
        let e = elem.trim().to_ascii_uppercase();
        if e != "NULL" {
          all_null_no_cast = false;
          break;
        }
      }
      if !all_null_no_cast {
        i = close + 1;
        continue;
      }
      // Skip if a `::<type>[]` cast follows the `]`.
      let mut after = close + 1;
      while after < n && bytes[after].is_ascii_whitespace() {
        after += 1;
      }
      if after + 2 <= n && &bytes[after..after + 2] == b"::" {
        i = close + 1;
        continue;
      }
      if emitted.insert(i) {
        let abs_s = start + i;
        let abs_e = start + close + 1;
        out.push(Diagnostic {
          code: "sql506",
          severity: Severity::Warning,
          message: "`ARRAY[NULL]` / `ARRAY[NULL, NULL, ...]` -- no element provides type information, so PG falls back to `text[]` or raises `cannot determine type of empty array`. Add an explicit cast: either inside (`NULL::int`) or on the result (`ARRAY[NULL]::int[]`).".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = close + 1;
    }
  }
}
