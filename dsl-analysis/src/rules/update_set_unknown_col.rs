//! sql042: `UPDATE <table> SET <col> = ...` where `<col>` is not in
//! the target table's catalog definition.
//!
//! Sibling of sql002 (unknown column inside SELECT). UPDATE statements
//! reach the catalog via `UpdateStmt.table` and assignments expose the
//! target column name, so checking the assignments against the
//! catalog's column list is straightforward.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql042"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Update(u) = &stmt.kind else {
      return;
    };
    if u.table.name.is_empty() {
      return;
    }
    let Some(t) = catalog.find_table(u.table.schema.as_deref(), &u.table.name) else {
      // sql001 already covers unresolved table.
      return;
    };
    let valid: std::collections::HashSet<String> = t.columns.iter().map(|c| c.name.to_ascii_lowercase()).collect();
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    for (target, _expr) in &u.assignments {
      // Strip qualifier if present: `t.col` -> `col`.
      let col = target.rsplit('.').next().unwrap_or(target);
      if !valid.contains(&col.to_ascii_lowercase()) {
        // Find `SET ... col` in the source to narrow the range.
        let upper = body.to_ascii_uppercase();
        let set_at = upper.find(" SET ").map(|i| i + 5).unwrap_or(0);
        let target_lower = target.to_ascii_lowercase();
        let body_lower = body.to_ascii_lowercase();
        let range = body_lower[set_at..]
          .find(&target_lower)
          .map(|r| {
            let abs_start = start + set_at + r;
            let abs_end = abs_start + target_lower.len();
            text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into())
          })
          .unwrap_or(stmt.range);
        out.push(Diagnostic {
          code: "sql042",
          severity: Severity::Error,
          message: format!("unknown column `{}` in UPDATE SET (table `{}`)", col, u.table.name),
          range,
        });
      }
    }
  }
}
