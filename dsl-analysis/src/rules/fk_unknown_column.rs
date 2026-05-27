//! sql185: `REFERENCES other(missing)` where `missing` isn't a
//! column on `other`. PG raises 42703 at runtime. Walks the
//! CREATE TABLE constraints + the catalog to validate every FK
//! target column exists.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::{Catalog, ConstraintKind};
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql185"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else { return };
    // FK info comes from the merged catalog where source_tables has
    // already harvested REFERENCES details from this CREATE TABLE.
    let Some(t) = catalog.find_table(ct.table.schema.as_deref(), &ct.table.name) else { return };
    for con in &t.constraints {
      if !matches!(con.kind, ConstraintKind::ForeignKey) {
        continue;
      }
      let Some(refs) = &con.references else { continue };
      let Some(target) =
        catalog.find_table(Some(&refs.schema), &refs.table).or_else(|| catalog.find_table(None, &refs.table))
      else {
        continue;
      };
      for col in &refs.columns {
        if target.columns.iter().any(|c| c.name.eq_ignore_ascii_case(col)) {
          continue;
        }
        // Range = whole CREATE TABLE statement; future work could
        // narrow to the FK's textual span.
        out.push(Diagnostic {
          code: "sql185",
          severity: Severity::Error,
          message: format!(
            "FK `{}` references missing column `{}.{}` -- column not declared on target table",
            con.name, refs.table, col
          ),
          range: stmt.range,
        });
      }
    }
    let _ = source;
  }
}
