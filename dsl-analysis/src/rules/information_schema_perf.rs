//! sql305: `FROM information_schema.<view>` -- the standard SQL
//! introspection views are usually 10-100x slower than the
//! equivalent `pg_catalog` queries because they're built on
//! cross-schema joins. Hint: for any non-portable script, query
//! `pg_catalog` directly.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql305"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let Some(at) = lower.find("information_schema.") else { return };
    let abs_s = start + at;
    let abs_e = abs_s + "information_schema.".len();
    out.push(Diagnostic {
      code: "sql305",
      severity: Severity::Hint,
      message: "information_schema views are slow vs pg_catalog (cross-schema joins) -- if portability isn't a concern, query pg_catalog (e.g. pg_class / pg_attribute) directly".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
