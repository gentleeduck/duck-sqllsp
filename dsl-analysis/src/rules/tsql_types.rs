//! sql629: SQL Server (T-SQL) data types that don't exist in PostgreSQL --
//! `NVARCHAR`, `NCHAR`, `DATETIME2`, `DATETIMEOFFSET`, `SMALLDATETIME`,
//! `UNIQUEIDENTIFIER`, `VARBINARY`, `NTEXT`, `IMAGE`, `SYSNAME`. PG rejects them
//! (42704). Each maps onto a native PG type. Complements the MySQL-types
//! (sql316) and Oracle-types lints.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const TYPES: &[(&str, &str)] = &[
  ("UNIQUEIDENTIFIER", "uuid"),
  ("DATETIMEOFFSET", "timestamptz"),
  ("SMALLDATETIME", "timestamp"),
  ("DATETIME2", "timestamp (or timestamptz)"),
  ("NVARCHAR", "varchar(n) / text"),
  ("VARBINARY", "bytea"),
  ("SYSNAME", "name / text"),
  ("NCHAR", "char(n)"),
  ("NTEXT", "text"),
  ("IMAGE", "bytea"),
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql629"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let n = ub.len();
    for &(ty, pg) in TYPES {
      let len = ty.len();
      let mut i = 0usize;
      while i + len <= n {
        if &ub[i..i + len] == ty.as_bytes()
          && (i == 0 || !is_word(ub[i - 1] as char))
          && (i + len == n || !is_word(ub[i + len] as char))
        {
          out.push(Diagnostic {
            code: "sql629",
            severity: Severity::Error,
            message: format!("`{ty}` is a SQL Server type with no PostgreSQL equivalent -- use `{pg}`"),
            range: crate::range_at(start + i, start + i + len),
          });
          i += len;
          continue;
        }
        i += 1;
      }
    }
  }
}
