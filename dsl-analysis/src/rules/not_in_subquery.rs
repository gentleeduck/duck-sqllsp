//! sql018: `NOT IN (subquery)` is dangerous when the subquery can return
//! NULL. Postgres treats `x NOT IN (NULL)` as UNKNOWN, so the predicate
//! never matches and the outer query silently returns zero rows.
//!
//! Heuristic: flag every literal `NOT IN (` followed by `SELECT`. We
//! don't try to prove the subquery is null-free -- the recommendation is
//! always to use `NOT EXISTS` or filter the subquery with `IS NOT NULL`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::{TextRange, TextSize};

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql018"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: u32 = stmt.range.start().into();
    let end: u32 = stmt.range.end().into();
    let end = (end as usize).min(source.len());
    let slice = &source[start as usize..end];
    let upper = slice.to_ascii_uppercase();

    // Walk every match of `NOT IN (`, then check the first non-space
    // word inside the paren is `SELECT`. Whole-word matching on the
    // `NOT` boundary avoids `CANNOT IN`.
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("NOT IN") {
      let i = from + rel;
      let prev_ok = i == 0 || !upper.as_bytes()[i - 1].is_ascii_alphanumeric();
      let after = i + "NOT IN".len();
      if !prev_ok {
        from = i + 1;
        continue;
      }
      // Skip whitespace then look for `(`
      let mut k = after;
      while k < slice.len() && (slice.as_bytes()[k] as char).is_whitespace() {
        k += 1;
      }
      if k >= slice.len() || slice.as_bytes()[k] != b'(' {
        from = after;
        continue;
      }
      // Skip whitespace inside paren
      let mut m = k + 1;
      while m < slice.len() && (slice.as_bytes()[m] as char).is_whitespace() {
        m += 1;
      }
      if upper[m..].starts_with("SELECT") {
        let s = TextSize::from(start + i as u32);
        let e = TextSize::from(start + after as u32);
        out.push(Diagnostic {
          code: "sql018",
          severity: Severity::Warning,
          message: "NOT IN (<subquery>) returns 0 rows when the subquery yields any NULL; \
                              prefer NOT EXISTS or add IS NOT NULL inside the subquery"
            .into(),
          range: TextRange::new(s, e),
        });
      }
      from = after;
    }
  }
}
