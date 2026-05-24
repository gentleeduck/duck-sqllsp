//! sql107: comparing a `jsonb` column to a text literal without
//! `::text` / `::jsonb` -- the comparison is always false because PG
//! treats the literal as jsonb.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql107"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Look for jsonb path operator `->>` followed by `=` then a
    // string literal -- common pattern that needs an explicit cast
    // when the user wants a text compare. Pattern: `... ->> 'k' = 'v'`.
    // We flag when `=` is followed directly by a string literal that
    // looks like jsonb (e.g. `'{"a":1}'`) -- otherwise too noisy.
    let Some(eq_at) = upper.find("=") else { return };
    let after = &body[eq_at + 1..];
    let after_trim = after.trim_start();
    if !after_trim.starts_with("'{") && !after_trim.starts_with("'[") {
      return;
    }
    // Walk back from `=` to find the column expression. If it
    // contains `->` or `->>`, flag.
    let before = &body[..eq_at];
    let upper_before = &upper[..eq_at];
    if !(upper_before.contains("->>") || upper_before.contains("->")) {
      return;
    }
    // Skip if user already cast (`::text =` or `::jsonb =` before
    // the equals).
    let trimmed_before = before.trim_end();
    if trimmed_before.ends_with("::text") || trimmed_before.ends_with("::jsonb") {
      return;
    }
    // Skip if RHS is `::jsonb` cast.
    if after_trim.contains("::jsonb") || after_trim.contains("::text") {
      return;
    }
    let abs_start = start + eq_at;
    let abs_end = start + eq_at + 1;
    out.push(Diagnostic {
      code: "sql107",
      severity: Severity::Hint,
      message: "jsonb expression compared to literal -- add `::text` or `::jsonb` cast to make intent explicit".into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}
