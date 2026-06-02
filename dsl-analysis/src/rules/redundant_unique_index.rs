//! sql168: `CREATE UNIQUE INDEX ... ON t (cols)` where `t` already
//! has a UNIQUE constraint on the same column set. PG already
//! enforces uniqueness via the constraint's implicit index.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql168"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Unknown { .. }) {
      return;
    }
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("CREATE UNIQUE INDEX") {
      return;
    }
    let Some(on_at) = upper.find(" ON ") else { return };
    let after_on = body[on_at + 4..].trim_start();
    let tbl: String = after_on.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
    let bare_tbl = tbl.rsplit('.').next().unwrap_or(&tbl).to_string();
    if bare_tbl.is_empty() {
      return;
    }
    let Some(open) = body.find('(') else { return };
    let Some(close) = body[open + 1..].find(')') else { return };
    let list = &body[open + 1..open + 1 + close];
    // Partial index (WHERE clause) covers only a subset of rows so it
    // is NOT redundant with an existing full UNIQUE/PK constraint.
    // Same for expression-only indexes (the col list may not match any
    // constraint anyway, but the WHERE clause is the canonical signal).
    let after_index_paren_upper = upper[open + 1 + close..].to_ascii_uppercase();
    if after_index_paren_upper
      .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
      .any(|w| w == "WHERE")
    {
      return;
    }
    let idx_cols: Vec<String> = list.split(',').map(|s| s.trim().trim_matches('"').to_ascii_lowercase()).collect();
    let Some(t) = catalog.find_table(None, &bare_tbl) else { return };
    // Find any UNIQUE / PK constraint with the exact same col set.
    for c in &t.constraints {
      if !matches!(c.kind, dsl_catalog::ConstraintKind::Unique | dsl_catalog::ConstraintKind::PrimaryKey) {
        continue;
      }
      if c.columns.len() != idx_cols.len() {
        continue;
      }
      let cols_lower: Vec<String> = c.columns.iter().map(|c| c.to_ascii_lowercase()).collect();
      if cols_lower.iter().all(|cc| idx_cols.contains(cc)) {
        let kind_name = match c.kind {
          dsl_catalog::ConstraintKind::PrimaryKey => "PRIMARY KEY",
          _ => "UNIQUE",
        };
        let abs_start = start;
        let abs_end = start + on_at;
        out.push(Diagnostic {
                    code: "sql168",
                    severity: Severity::Hint,
                    message: format!("CREATE UNIQUE INDEX on `{bare_tbl}({})` -- the existing {kind_name} constraint already creates a unique index on the same columns", list.trim()),
                    range: crate::range_at(abs_start, abs_end),
                });
        return;
      }
    }
  }
}
