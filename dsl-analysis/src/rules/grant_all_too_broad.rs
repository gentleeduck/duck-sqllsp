//! sql291: `GRANT ALL PRIVILEGES ON ...` (or bare `GRANT ALL`) --
//! the principle of least privilege says enumerate. Hint: list the
//! specific privileges (SELECT / INSERT / UPDATE / DELETE / USAGE /
//! EXECUTE / TRIGGER / etc).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql291"
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
    if !trim.starts_with("GRANT") {
      return;
    }
    let has_all =
      upper.contains("GRANT ALL") && (upper.contains("GRANT ALL PRIVILEGES") || upper.contains("GRANT ALL ON"));
    if !has_all {
      return;
    }
    let Some(at) = upper.find("GRANT ALL") else { return };
    let abs_s = start + at;
    let abs_e = abs_s + "GRANT ALL".len();
    out.push(Diagnostic {
      code: "sql291",
      severity: Severity::Hint,
      message: "GRANT ALL [PRIVILEGES] is overly broad -- enumerate just the privileges you need (SELECT/INSERT/UPDATE/USAGE/EXECUTE/etc) per least-privilege".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
