//! sql119: `SET TRANSACTION ISOLATION LEVEL ...` must be the **first**
//! statement after `BEGIN` -- otherwise PG ignores it. Catches the
//! mistake of putting it after a SELECT.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql119"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("SET TRANSACTION ISOLATION") {
      return;
    }
    // Walk backwards in source to find the most recent BEGIN /
    // COMMIT / ROLLBACK / start-of-file. If any non-empty statement
    // (other than SET / BEGIN) appears between that BEGIN and us,
    // the SET TRANSACTION ISOLATION is too late.
    let before_upper = source[..start].to_ascii_uppercase();
    let mut begin_at = None;
    for token in ["BEGIN", "START TRANSACTION"] {
      if let Some(pos) = before_upper.rfind(token) {
        begin_at = match begin_at {
          None => Some(pos),
          Some(p) => Some(p.max(pos)),
        };
      }
    }
    let begin_at = match begin_at {
      Some(p) => p,
      None => return,
    };
    // If a COMMIT / ROLLBACK appears after the BEGIN, we're not in
    // that transaction anymore -- skip.
    if let Some(end_at) = before_upper[begin_at..].find("COMMIT").or_else(|| before_upper[begin_at..].find("ROLLBACK"))
    {
      if end_at > 0 {
        return;
      }
    }
    // Look for any executable statement in between -- a `;` that
    // closes something other than BEGIN/START TRANSACTION/SET.
    let between = &source[begin_at..start];
    let stmts: Vec<&str> = between.split(';').collect();
    for s in &stmts[1..] {
      // skip the BEGIN itself
      let t = s.trim().to_ascii_uppercase();
      if t.is_empty() {
        continue;
      }
      if t.starts_with("SET TRANSACTION") || t.starts_with("SET LOCAL") {
        continue;
      }
      // Found a real statement before this SET TRANSACTION ISOLATION.
      let leading = upper.len() - trimmed.len();
      let abs_start = start + leading;
      let abs_end = start + leading + 25; // "SET TRANSACTION ISOLATION"
      out.push(Diagnostic {
                code: "sql119",
                severity: Severity::Hint,
                message: "SET TRANSACTION ISOLATION LEVEL must be the first statement in the transaction -- earlier statements have already locked the level".into(),
                range: text_size::TextRange::new(
                    (abs_start as u32).into(),
                    (abs_end as u32).into(),
                ),
            });
      return;
    }
  }
}
