//! sql196: `REFERENCES other(col)` where `other.col` is not the
//! target of a PRIMARY KEY or UNIQUE constraint / unique index.
//! PG raises 42830 "there is no unique constraint matching given
//! keys for referenced table" at CREATE TABLE.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::{Catalog, ConstraintKind};
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql196"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else { return };
    let Some(self_t) = catalog.find_table(ct.table.schema.as_deref(), &ct.table.name) else { return };
    for con in &self_t.constraints {
      if !matches!(con.kind, ConstraintKind::ForeignKey) {
        continue;
      }
      let Some(refs) = &con.references else { continue };
      let target =
        catalog.find_table(Some(&refs.schema), &refs.table).or_else(|| catalog.find_table(None, &refs.table));
      let Some(target) = target else { continue };
      let mut needed: Vec<String> = refs.columns.iter().map(|s| s.to_ascii_lowercase()).collect();
      needed.sort();
      let mut found = false;
      for c in &target.constraints {
        if !matches!(c.kind, ConstraintKind::PrimaryKey | ConstraintKind::Unique) {
          continue;
        }
        let mut have: Vec<String> = c.columns.iter().map(|s| s.to_ascii_lowercase()).collect();
        have.sort();
        if have == needed {
          found = true;
          break;
        }
      }
      if !found {
        for idx in &target.indexes {
          if !idx.unique {
            continue;
          }
          let mut have: Vec<String> = idx.columns.iter().map(|s| s.to_ascii_lowercase()).collect();
          have.sort();
          if have == needed {
            found = true;
            break;
          }
        }
      }
      if found {
        continue;
      }
      out.push(Diagnostic {
        code: "sql196",
        severity: Severity::Error,
        message: format!(
          "FK `{}` -> `{}({})` -- target columns not covered by PK/UNIQUE -- PG raises 42830",
          con.name,
          refs.table,
          refs.columns.join(", "),
        ),
        range: stmt.range,
      });
    }
    let _ = source;
  }
}
