//! sql246: `INSERT ... ON CONFLICT DO NOTHING` (without the column
//! list / constraint name to scope it). Without an inference target
//! PG swallows ANY constraint violation: PK clash, UNIQUE, EXCLUDE,
//! even CHECK. Almost always the author wanted to ignore only the
//! specific dup-key case. Suggest naming the conflict target.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql246"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    // Strip comments + strings so a `-- ON CONFLICT with ...` header
    // doesn't hijack the keyword anchor.
    let body_owned = crate::textutil::strip_comments_only(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(oc_at) = upper.find("ON CONFLICT") else { return };
    let post_oc = &upper[oc_at + "ON CONFLICT".len()..];
    let trimmed = post_oc.trim_start();
    // Scoped via ON CONSTRAINT or (col, ...) -- fine.
    if trimmed.starts_with("ON CONSTRAINT") || trimmed.starts_with('(') {
      return;
    }
    // Must be followed by DO NOTHING (else DO UPDATE form which still benefits but is intentional).
    if !post_oc.contains("DO NOTHING") {
      return;
    }
    let abs_s = start + oc_at;
    let abs_e = abs_s + "ON CONFLICT DO NOTHING".len().min(body.len() - oc_at);
    out.push(Diagnostic {
      code: "sql246",
      severity: Severity::Hint,
      message: "ON CONFLICT DO NOTHING (no target) swallows EVERY constraint violation, not just dup-key -- scope with `(col)` or `ON CONSTRAINT <name>`".into(),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
