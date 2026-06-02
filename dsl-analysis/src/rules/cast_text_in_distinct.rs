//! sql138: `SELECT DISTINCT (col)::text FROM t` -- casting to text
//! inside DISTINCT throws away the typed comparison and runs a
//! string compare, almost always wrong.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql138"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    // Need DISTINCT (not DISTINCT ON) and ::text cast in projection
    // before FROM.
    let Some(dist) = upper.find("DISTINCT") else { return };
    if upper[dist..].starts_with("DISTINCT ON") {
      return;
    }
    let from_at = upper.find(" FROM ").unwrap_or(upper.len());
    if dist >= from_at {
      return;
    }
    let proj = &upper[dist + 8..from_at];
    if !proj.contains("::TEXT") {
      return;
    }
    let cast_rel = proj.find("::TEXT").unwrap();
    let abs_start = start + dist + 8 + cast_rel;
    let abs_end = abs_start + 6;
    out.push(Diagnostic {
      code: "sql138",
      severity: Severity::Hint,
      message:
        "::text cast inside DISTINCT throws away typed comparison -- DISTINCT runs string compare on the cast result"
          .into(),
      range: crate::range_at(abs_start, abs_end),
    });
  }
}
