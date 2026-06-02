//! sql413: `expr || NULL` / `NULL || expr` -- the `||` operator
//! returns NULL when either operand is NULL, so any literal NULL in a
//! string-concatenation chain silently drops the whole expression to
//! NULL. Use `concat()` (NULL-as-empty-string) or
//! `coalesce(part, '')` when that's actually what you want.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql413"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    let mut emitted = false;
    while i + 1 < n {
      if bytes[i] != b'|' || bytes[i + 1] != b'|' {
        i += 1;
        continue;
      }
      // Skip whitespace forward; check for NULL keyword.
      let mut j = i + 2;
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      let right_null = at_null_keyword(bytes, j);
      // Skip whitespace backward; check for NULL on the left.
      let mut k = i;
      while k > 0 && bytes[k - 1].is_ascii_whitespace() {
        k -= 1;
      }
      let left_null = ends_with_null_keyword(bytes, k);
      if right_null || left_null {
        let abs_s = start + i;
        let abs_e = start + (i + 2);
        out.push(Diagnostic {
          code: "sql413",
          severity: Severity::Warning,
          message: "`||` with a NULL operand returns NULL; use `concat(...)` to treat NULL as empty or `coalesce(part, '')`".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
        emitted = true;
        i += 2;
        continue;
      }
      i += 2;
    }
    let _ = emitted;
  }
}

fn at_null_keyword(bytes: &[u8], i: usize) -> bool {
  i + 4 <= bytes.len()
    && &bytes[i..i + 4] == b"NULL"
    && (i == 0 || !is_word(bytes[i - 1] as char))
    && (i + 4 == bytes.len() || !is_word(bytes[i + 4] as char))
}

fn ends_with_null_keyword(bytes: &[u8], end: usize) -> bool {
  if end < 4 {
    return false;
  }
  let start = end - 4;
  &bytes[start..end] == b"NULL"
    && (start == 0 || !is_word(bytes[start - 1] as char))
    && (end == bytes.len() || !is_word(bytes[end] as char))
}

