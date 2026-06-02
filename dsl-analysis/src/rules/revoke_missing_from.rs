//! sql309: `REVOKE SELECT ON foo;` -- missing `FROM <role>`. PG
//! raises 42601 at parse time. Catches the typo where author
//! ported GRANT syntax but forgot to flip TO to FROM.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql309"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("REVOKE") {
      return;
    }
    if upper.contains(" FROM ") {
      return;
    }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql309",
      severity: Severity::Error,
      message: "REVOKE missing `FROM <role>` -- PG raises 42601; typo for `TO` from GRANT syntax?".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
