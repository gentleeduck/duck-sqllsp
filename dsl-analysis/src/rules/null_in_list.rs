//! sql437: `WHERE NULL IN (1, 2, 3)` -- the LHS literal is NULL, so
//! the IN expression evaluates to NULL (not TRUE/FALSE) regardless
//! of the list contents. PG treats a NULL WHERE result as failure,
//! so the row is dropped -- the whole query returns nothing. Almost
//! certainly a typo (the user meant a column name on the LHS) or a
//! leftover placeholder. Also covers `NULL NOT IN (...)` which
//! similarly always evaluates to NULL.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql437"
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
    while i + 4 <= n {
      // Word-bounded NULL.
      if &ub[i..i + 4] != b"NULL"
        || (i > 0 && is_word(ub[i - 1] as char))
        || (i + 4 < n && is_word(ub[i + 4] as char))
      {
        i += 1;
        continue;
      }
      // Skip whitespace, then look for IN or NOT IN.
      let mut j = i + 4;
      while j < n && bytes[j].is_ascii_whitespace() {
        j += 1;
      }
      let (is_not_in, after_op) = if j + 3 <= n
        && &ub[j..j + 3] == b"NOT"
        && (j + 3 == n || !is_word(ub[j + 3] as char))
      {
        let mut k = j + 3;
        while k < n && bytes[k].is_ascii_whitespace() {
          k += 1;
        }
        if k + 2 <= n && &ub[k..k + 2] == b"IN" && (k + 2 == n || !is_word(ub[k + 2] as char)) {
          (true, k + 2)
        } else {
          i += 1;
          continue;
        }
      } else if j + 2 <= n && &ub[j..j + 2] == b"IN" && (j + 2 == n || !is_word(ub[j + 2] as char)) {
        (false, j + 2)
      } else {
        i += 1;
        continue;
      };
      // After IN must come `(` (after optional whitespace).
      let mut k = after_op;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += 1;
        continue;
      }
      let op_text = if is_not_in { "NOT IN" } else { "IN" };
      let abs_s = start + i;
      let abs_e = start + k + 1;
      out.push(Diagnostic {
        code: "sql437",
        severity: Severity::Warning,
        message: format!(
          "`NULL {op_text} (...)` always evaluates to NULL (not TRUE/FALSE) -- in WHERE / ON / HAVING the row is dropped, so the predicate never matches; the LHS is probably meant to be a column name"
        ),
        range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      i = k + 1;
    }
  }
}
