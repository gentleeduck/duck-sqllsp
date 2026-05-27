//! sql498: `WHERE col SIMILAR TO 'pattern'` -- the PG-specific
//! SIMILAR TO operator is a third-rail SQL-standard regex variant
//! that's neither LIKE nor POSIX regex. It's slower than POSIX
//! regex (the `~` operator) in many cases and rarely understood
//! outside PG. Prefer:
//!   * `LIKE '...'` for simple wildcard (`%` / `_`) patterns
//!   * `~ '...'` (POSIX regex) for full regular expressions

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql498"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut emitted: std::collections::HashSet<usize> = std::collections::HashSet::new();
    // Find `SIMILAR` followed by `TO`.
    let mut i = 0usize;
    while i + 7 <= n {
      if !(&ub[i..i + 7] == b"SIMILAR" && (i == 0 || !is_word(ub[i - 1] as char)) && (i + 7 == n || !is_word(ub[i + 7] as char))) {
        i += 1;
        continue;
      }
      let mut k = i + 7;
      while k < n && ub[k].is_ascii_whitespace() {
        k += 1;
      }
      if k + 2 > n || &ub[k..k + 2] != b"TO" || (k + 2 < n && is_word(ub[k + 2] as char)) {
        i += 7;
        continue;
      }
      // Detect a preceding NOT.
      let mut s = i;
      let mut l = i;
      while l > 0 && ub[l - 1].is_ascii_whitespace() {
        l -= 1;
      }
      if l >= 3 && &ub[l - 3..l] == b"NOT" && (l == 3 || !is_word(ub[l - 4] as char)) {
        s = l - 3;
      }
      if emitted.insert(s) {
        let abs_s = start + s;
        let abs_e = start + k + 2;
        out.push(Diagnostic {
          code: "sql498",
          severity: Severity::Hint,
          message: "`SIMILAR TO` is a PG-specific SQL-standard regex variant that's slower than POSIX regex and rarely used outside PG. Use `LIKE '...'` for wildcard patterns (`%`/`_`) or `~ '...'` (POSIX regex) for full regular expressions.".into(),
          range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
      }
      i = k + 2;
    }
  }
}
