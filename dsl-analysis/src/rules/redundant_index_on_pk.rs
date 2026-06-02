//! sql167: `CREATE INDEX ... ON t (pk_col)` where `pk_col` is the
//! primary key of `t`. PRIMARY KEY already creates a unique B-tree
//! index, so the explicit one is duplicate storage + maintenance cost.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql167"
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
    let is_index = trimmed.starts_with("CREATE INDEX") || trimmed.starts_with("CREATE UNIQUE INDEX");
    if !is_index {
      return;
    }
    // Parse: CREATE [UNIQUE] INDEX [name] ON <table> (<col>)
    let Some(on_at) = upper.find(" ON ") else { return };
    let after_on = body[on_at + 4..].trim_start();
    let tbl: String = after_on.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.').collect();
    let bare_tbl = tbl.rsplit('.').next().unwrap_or(&tbl).to_string();
    if bare_tbl.is_empty() {
      return;
    }
    // Find the column list `(col)`.
    let Some(open) = body.find('(') else { return };
    let Some(close) = body[open + 1..].find(')') else { return };
    let list = &body[open + 1..open + 1 + close];
    let cols: Vec<&str> = list.split(',').map(|s| s.trim().trim_matches('"')).collect();
    if cols.len() != 1 {
      return;
    }
    let idx_col = cols[0].to_string();
    // Look up table in catalog -- if PK on exactly this column,
    // flag.
    let Some(t) = catalog.find_table(None, &bare_tbl) else { return };
    let pk = t.constraints.iter().find(|c| matches!(c.kind, dsl_catalog::ConstraintKind::PrimaryKey));
    let Some(pk) = pk else { return };
    if pk.columns.len() != 1 {
      return;
    }
    if !pk.columns[0].eq_ignore_ascii_case(&idx_col) {
      return;
    }
    let abs_start = start;
    let abs_end = start + on_at;
    out.push(Diagnostic {
            code: "sql167",
            severity: Severity::Hint,
            message: format!("CREATE INDEX on `{bare_tbl}({idx_col})` -- the PRIMARY KEY already maintains a unique B-tree index on this column; the explicit one duplicates storage"),
            range: crate::range_at(abs_start, abs_end),
        });
  }
}
