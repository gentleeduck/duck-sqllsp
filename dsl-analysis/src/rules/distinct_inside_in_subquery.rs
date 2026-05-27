//! sql471: `WHERE x IN (SELECT DISTINCT y FROM t)` -- the DISTINCT
//! inside an IN-subquery is wasted work. The IN operator already
//! treats the subquery's result as a set: row equality against any
//! occurrence of `y` succeeds regardless of how many times `y`
//! appears. Drop the DISTINCT to let the planner pick the better
//! plan (often a hash semi-join).
//!
//! Same for `NOT IN (SELECT DISTINCT ...)`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql471"
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
    let bytes = cleaned.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 2 <= n {
      // Match word-bounded IN.
      if !(&ub[i..i + 2] == b"IN" && (i == 0 || !is_word(ub[i - 1] as char)) && (i + 2 == n || !is_word(ub[i + 2] as char))) {
        i += 1;
        continue;
      }
      let mut k = i + 2;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      if k >= n || bytes[k] != b'(' {
        i += 2;
        continue;
      }
      k += 1;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      // Expect SELECT
      if !(k + 6 <= n && &ub[k..k + 6] == b"SELECT" && (k + 6 == n || !is_word(ub[k + 6] as char))) {
        i += 2;
        continue;
      }
      k += 6;
      while k < n && bytes[k].is_ascii_whitespace() {
        k += 1;
      }
      // Expect DISTINCT (no ON).
      if !(k + 8 <= n && &ub[k..k + 8] == b"DISTINCT" && (k + 8 == n || !is_word(ub[k + 8] as char))) {
        i += 2;
        continue;
      }
      // Skip when followed by `ON` -- DISTINCT ON has semantic meaning.
      let mut p = k + 8;
      while p < n && bytes[p].is_ascii_whitespace() {
        p += 1;
      }
      if p + 2 <= n && &ub[p..p + 2] == b"ON" && (p + 2 == n || !is_word(ub[p + 2] as char)) {
        i += 2;
        continue;
      }
      let abs_s = start + i;
      let abs_e = start + k + 8;
      out.push(Diagnostic {
        code: "sql471",
        severity: Severity::Hint,
        message: "DISTINCT inside an `IN (SELECT ...)` subquery is redundant -- IN treats the subquery's result as a set anyway; drop the DISTINCT and let the planner pick the better plan (often a hash semi-join)".into(),
        range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      i = k + 8;
    }
  }
}
