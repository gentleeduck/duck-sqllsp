//! sql512: table-level PK / UNIQUE / FK source constraint references a
//! column that isn't declared on this table. PG raises 42703 at
//! `CREATE TABLE` time. Catches typos like:
//!
//!   CREATE TABLE t (
//!     id int,
//!     CONSTRAINT pk_t PRIMARY KEY (idd)   -- typo
//!   );
//!
//! sql185 covers the *target* side of an FK (referenced table's column).
//! This rule covers the *source* side (the column on the table being
//! defined). CHECK bodies are out of scope -- they accept arbitrary
//! expressions and would need full expression resolution.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::{Catalog, ConstraintKind};
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql512"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, _source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(ct) = &stmt.kind else { return };
    let Some(t) = catalog.find_table(ct.table.schema.as_deref(), &ct.table.name) else { return };
    let known: Vec<&str> = t.columns.iter().map(|c| c.name.as_str()).collect();
    for con in &t.constraints {
      if !matches!(con.kind, ConstraintKind::PrimaryKey | ConstraintKind::Unique | ConstraintKind::ForeignKey) {
        continue;
      }
      for col in &con.columns {
        if known.iter().any(|k| k.eq_ignore_ascii_case(col)) {
          continue;
        }
        let kind_str = match con.kind {
          ConstraintKind::PrimaryKey => "PRIMARY KEY",
          ConstraintKind::Unique => "UNIQUE",
          ConstraintKind::ForeignKey => "FOREIGN KEY",
          ConstraintKind::Check => "CHECK",
        };
        out.push(Diagnostic {
          code: "sql512",
          severity: Severity::Error,
          message: format!("{kind_str} constraint `{}` references column `{col}` not declared on this table", con.name),
          range: stmt.range,
        });
      }
    }
  }
}
