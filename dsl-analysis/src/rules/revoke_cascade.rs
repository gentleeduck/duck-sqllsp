//! sql287: `REVOKE ... CASCADE` on a privilege the grantee may
//! have re-granted. CASCADE recursively revokes from every onward
//! grantee -- a chain reaction. Hint: confirm intent or use
//! `RESTRICT` (the default) so failures are explicit.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql287"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim_start();
    if !trim.starts_with("REVOKE") {
      return;
    }
    if !upper.contains("CASCADE") {
      return;
    }
    let Some(at) = upper.find("CASCADE") else { return };
    let abs_s = start + at;
    let abs_e = abs_s + "CASCADE".len();
    out.push(Diagnostic {
      code: "sql287",
      severity: Severity::Hint,
      message: "REVOKE ... CASCADE recursively revokes from every onward grantee -- confirm intent or use RESTRICT (default) so dependent grants fail loudly".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
