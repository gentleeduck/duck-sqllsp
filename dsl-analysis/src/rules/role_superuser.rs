//! sql572: `CREATE ROLE deploy SUPERUSER` / `ALTER ROLE app SUPERUSER` --
//! granting the SUPERUSER attribute. A superuser bypasses every permission
//! check (and can read/write any file the server account can), so it should
//! be reserved for the bootstrap/admin role. Grant the specific privileges
//! (or attributes like CREATEDB / CREATEROLE) the role actually needs.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql572"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    // SUPERUSER is only meaningful in role DDL.
    if !(upper.contains("ROLE") || upper.contains("USER")) {
      return;
    }
    let ub = upper.as_bytes();
    let n = ub.len();
    let mut i = 0usize;
    while i + 9 <= n {
      // Word-bounded SUPERUSER; the leading boundary excludes NOSUPERUSER.
      if &ub[i..i + 9] == b"SUPERUSER"
        && (i == 0 || !is_word(ub[i - 1] as char))
        && (i + 9 == n || !is_word(ub[i + 9] as char))
      {
        out.push(Diagnostic {
          code: "sql572",
          severity: Severity::Warning,
          message: "granting SUPERUSER -- it bypasses all permission checks; grant only the privileges the role needs".into(),
          range: crate::range_at(start + i, start + i + 9),
        });
        return;
      }
      i += 1;
    }
  }
}
