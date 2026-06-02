//! sql013: UPDATE or DELETE without a WHERE clause. Almost always a bug
//! waiting to clear out a whole table.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql013"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (stmt_start, stmt_end) = crate::stmt_bounds(stmt, source);
    let body = &source[stmt_start..stmt_end];
    let upper = body.to_ascii_uppercase();
    match &stmt.kind {
      StatementKind::Update(u) if u.where_clause.is_none() => {
        let range = first_word_range(stmt_start, &upper, "UPDATE").unwrap_or(stmt.range);
        out.push(Diagnostic {
          code: "sql013",
          severity: Severity::Warning,
          message: format!("UPDATE without WHERE will modify every row in `{}`", u.table.name),
          range,
        });
      },
      StatementKind::Delete(d) if d.where_clause.is_none() => {
        let range = first_word_range(stmt_start, &upper, "DELETE").unwrap_or(stmt.range);
        out.push(Diagnostic {
          code: "sql013",
          severity: Severity::Warning,
          message: format!("DELETE without WHERE will remove every row in `{}`", d.table.name),
          range,
        });
      },
      _ => {},
    }
  }
}

fn first_word_range(stmt_start: usize, upper: &str, needle: &str) -> Option<text_size::TextRange> {
  let rel = upper.find(needle)?;
  let abs_start = stmt_start + rel;
  let abs_end = abs_start + needle.len();
  Some(crate::range_at(abs_start, abs_end))
}
