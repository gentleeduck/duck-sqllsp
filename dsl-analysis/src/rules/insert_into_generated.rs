//! sql178: `INSERT INTO t (id, ...) VALUES (123, ...)` where `id`
//! is GENERATED ALWAYS AS IDENTITY. PG rejects unless the statement
//! says `OVERRIDING SYSTEM VALUE`. Detect by checking the catalog
//! column's default for `GENERATED ALWAYS`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql178"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else { return };
    if ins.columns.is_empty() {
      return;
    }
    let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) else { return };

    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if upper.contains("OVERRIDING SYSTEM VALUE") {
      return;
    }
    for col_name in &ins.columns {
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(col_name)) else { continue };
      let default = col.default.as_deref().unwrap_or("");
      if !default.to_ascii_uppercase().contains("GENERATED ALWAYS") {
        continue;
      }
      // Range = the column name in the col list.
      let col_at = body.to_ascii_lowercase().find(&col_name.to_ascii_lowercase()).unwrap_or(0);
      let abs_s = start + col_at;
      let abs_e = abs_s + col_name.len();
      out.push(Diagnostic {
        code: "sql178",
        severity: Severity::Error,
        message: format!(
          "`{}` is GENERATED ALWAYS -- omit it from the INSERT or add `OVERRIDING SYSTEM VALUE`",
          col_name
        ),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
