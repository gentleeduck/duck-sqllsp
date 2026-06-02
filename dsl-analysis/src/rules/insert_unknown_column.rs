//! sql349: `INSERT INTO t (col_list)` lists a column not in the
//! target table's catalog. Catches typos in INSERT statements.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql349"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else { return };
    let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) else { return };
    if ins.columns.is_empty() {
      return;
    }
    let (start, body) = crate::stmt_body(stmt, source);
    for col in &ins.columns {
      if t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(col)) {
        continue;
      }
      let Some(at) = crate::textutil::find_word(body, col) else { continue };
      let abs_s = start + at;
      let abs_e = abs_s + col.len();
      out.push(Diagnostic {
        code: "sql349",
        severity: Severity::Error,
        message: format!(
          "column `{col}` is not a column of `{}.{}`",
          ins.table.schema.as_deref().unwrap_or("public"),
          ins.table.name
        ),
        range: crate::range_at(abs_s, abs_e),
      });
    }
  }
}
