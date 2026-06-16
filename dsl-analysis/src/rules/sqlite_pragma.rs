//! sql635: a `PRAGMA ...` statement. PRAGMA is SQLite's mechanism for reading
//! and setting database options (e.g. `PRAGMA foreign_keys = ON`). PostgreSQL
//! has no PRAGMA and rejects it; configure the session with `SET`, the cluster
//! with `ALTER SYSTEM` / `postgresql.conf`, and note that foreign keys are
//! always enforced in PG.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql635"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let u = upper.trim_start();
    let lead = upper.len() - u.len();
    if u.starts_with("PRAGMA") && u.as_bytes().get(6).is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_') {
      out.push(Diagnostic {
        code: "sql635",
        severity: Severity::Error,
        message: "`PRAGMA` is a SQLite statement -- PostgreSQL has no PRAGMA; use `SET` (session), `ALTER SYSTEM` / postgresql.conf (cluster)".into(),
        range: crate::range_at(start + lead, start + lead + 6),
      });
    }
  }
}
