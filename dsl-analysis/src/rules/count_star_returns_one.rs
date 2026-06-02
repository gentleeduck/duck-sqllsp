//! sql154: `SELECT count(*) FROM t WHERE ...` (no GROUP BY) returns
//! **one row** even when the WHERE matches nothing -- count() is an
//! aggregate over the empty set = 0. Common gotcha when porting from
//! per-row languages where "no rows" expected.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql154"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("SELECT") {
      return;
    }
    // Only flag the lone-aggregate pattern: projection is just
    // `count(...)` (possibly with `AS alias`). Multi-projection
    // queries probably have other columns the user wants.
    let Some(from_at) = upper.find(" FROM ") else { return };
    let proj = &upper[6..from_at].trim();
    let count_call = proj.starts_with("COUNT(") || proj.starts_with("COUNT (");
    if !count_call {
      return;
    }
    // Already GROUP BY? Skip.
    if upper.contains(" GROUP BY ") {
      return;
    }
    // Already a WHERE that's just `false` / `1=0`? Skip noise.
    // No WHERE at all -> not interesting; only flag when WHERE is
    // present (the gotcha is `count(*) WHERE matches_nothing`).
    if !upper.contains(" WHERE ") {
      return;
    }
    // Locate the projection `count(...)` for the diagnostic range.
    let body_lower = body;
    let Some(rel) = body_lower.to_ascii_uppercase().find("COUNT") else { return };
    let abs_start = start + rel;
    let abs_end = abs_start + 5;
    out.push(Diagnostic {
            code: "sql154",
            severity: Severity::Hint,
            message: "count(*) without GROUP BY returns 1 row (the count) even when WHERE matches nothing -- check `count(*) = 0` if you want 'no matches'".into(),
            range: crate::range_at(abs_start, abs_end),
        });
  }
}
