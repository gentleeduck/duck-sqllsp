//! sql598: `USE mydb` -- the MySQL / SQL Server command to switch the current
//! database. PostgreSQL has no `USE` statement: a connection is bound to one
//! database for its lifetime. Switch with the psql meta-command `\c dbname`,
//! or point the connection string at the target database.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql598"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let u = upper.trim_start();
    let lead = upper.len() - u.len();
    if u.starts_with("USE") && u.as_bytes().get(3).is_some_and(|&b| b.is_ascii_whitespace()) {
      out.push(Diagnostic {
        code: "sql598",
        severity: Severity::Error,
        message: "`USE` is a MySQL/SQL Server command -- PostgreSQL has no USE; reconnect with psql `\\c dbname` or change the connection string".into(),
        range: crate::range_at(start + lead, start + lead + 3),
      });
    }
  }
}
