//! sql241: `CREATE [OR REPLACE] VIEW v AS SELECT * FROM t` -- the
//! view's column set is frozen at CREATE time. Adding a column to t
//! later does NOT appear in v, and dropping a column from t breaks
//! the view at the next OR REPLACE. Hint: list columns explicitly.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql241"
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
    if !(trim.starts_with("CREATE VIEW")
      || trim.starts_with("CREATE OR REPLACE VIEW")
      || trim.starts_with("CREATE MATERIALIZED VIEW")
      || trim.starts_with("CREATE OR REPLACE MATERIALIZED VIEW"))
    {
      return;
    }
    let Some(as_at) = upper.find(" AS ") else { return };
    let after_as = as_at + " AS ".len();
    let rest = body[after_as..].trim_start();
    let rest_upper = rest.to_ascii_uppercase();
    if !rest_upper.starts_with("SELECT") {
      return;
    }
    let after_sel = 6;
    let proj_end = rest_upper.find(" FROM ").unwrap_or(rest.len());
    let proj = rest[after_sel..proj_end].trim();
    if proj != "*" {
      return;
    }
    let star_off = rest.find('*').unwrap_or(0);
    let abs_s = start + after_as + (body[after_as..].len() - rest.len()) + star_off;
    let abs_e = abs_s + 1;
    out.push(Diagnostic {
      code: "sql241",
      severity: Severity::Hint,
      message: "CREATE VIEW with SELECT * -- view column set freezes at create-time; list explicit columns to avoid silent drift when base table changes".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
