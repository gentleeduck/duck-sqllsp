//! sql153: `now() + 1`, `created_at + 30` -- integer added to a
//! timestamp uses *days*, which is rarely what's meant. Use an
//! explicit `interval '1 day'` / `interval '30 minutes'`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql153"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let bytes = body.as_bytes();
    let n = bytes.len();
    // Pattern: `now()` or `current_timestamp` or `current_date`
    // followed by ` + <digits>` or ` - <digits>` -- with no
    // `INTERVAL` keyword between.
    for needle in &["NOW()", "CURRENT_TIMESTAMP", "CURRENT_DATE"] {
      let mut i = 0;
      while let Some(rel) = upper[i..].find(needle) {
        let at = i + rel;
        let after = at + needle.len();
        let mut j = after;
        while j < n && bytes[j].is_ascii_whitespace() {
          j += 1;
        }
        if j < n && (bytes[j] == b'+' || bytes[j] == b'-') {
          let op = bytes[j];
          j += 1;
          while j < n && bytes[j].is_ascii_whitespace() {
            j += 1;
          }
          let digits_start = j;
          while j < n && bytes[j].is_ascii_digit() {
            j += 1;
          }
          if j > digits_start {
            // Next non-ws token must not be INTERVAL.
            let mut k = j;
            while k < n && bytes[k].is_ascii_whitespace() {
              k += 1;
            }
            let rest = &upper[k..];
            if rest.starts_with("INTERVAL") {
              i = at + 1;
              continue;
            }
            let abs_start = start + at;
            let abs_end = start + j;
            out.push(Diagnostic {
              code: "sql153",
              severity: Severity::Hint,
              message: format!(
                "`{} {} <int>` adds *days* -- use `INTERVAL '{} day'` (or 'minutes' / 'hours') so the unit is explicit",
                needle,
                op as char,
                &body[digits_start..j],
              ),
              range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
            });
            return;
          }
        }
        i = at + 1;
      }
    }
  }
}
