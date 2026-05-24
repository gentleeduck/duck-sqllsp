//! sql130: multiple `TRUNCATE` statements in one transaction. PG
//! supports `TRUNCATE a, b, c` directly -- batching gets a single
//! AccessExclusiveLock acquisition and one rewrite.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql130"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("TRUNCATE") {
      return;
    }
    // Are we inside a BEGIN..COMMIT window?
    let before_upper = source[..start].to_ascii_uppercase();
    let last_begin = before_upper.rfind("BEGIN").or_else(|| before_upper.rfind("START TRANSACTION"));
    let Some(begin_at) = last_begin else { return };
    if before_upper[begin_at..].find("COMMIT").is_some() || before_upper[begin_at..].find("ROLLBACK").is_some() {
      return;
    }
    // Look back from begin_at..start for another TRUNCATE.
    let between = &before_upper[begin_at..start];
    if !between.contains("TRUNCATE") {
      return;
    }
    let leading = upper.len() - trimmed.len();
    let abs_start = start + leading;
    let abs_end = abs_start + 8;
    out.push(Diagnostic {
            code: "sql130",
            severity: Severity::Warning,
            message: "multiple TRUNCATE statements in one transaction -- combine into `TRUNCATE a, b, c` for a single lock acquisition".into(),
            range: text_size::TextRange::new(
                (abs_start as u32).into(),
                (abs_end as u32).into(),
            ),
        });
  }
}
