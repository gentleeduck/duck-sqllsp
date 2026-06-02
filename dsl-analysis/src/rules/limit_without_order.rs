//! sql051: `LIMIT` without `ORDER BY` produces non-deterministic rows.
//!
//! PG's planner is free to return any subset matching the predicate
//! when no ORDER BY pins the row order. Warn so the author makes the
//! ordering explicit (or adds a comment if they really want the random
//! sample).

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql051"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(_) = &stmt.kind else {
      return;
    };
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !crate::textutil::contains_word(&upper, "LIMIT") {
      return;
    }
    if crate::textutil::contains_word(&upper, "ORDER BY") {
      return;
    }
    // Single-row LIMIT 1 with a UNIQUE-ish predicate is often
    // intentional. Skip when LIMIT 1 appears (common pattern) --
    // but only that exact case.
    if upper.contains(" LIMIT 1") && !upper.contains(" LIMIT 10") {
      return;
    }
    // Narrow the diagnostic to the LIMIT keyword itself.
    let rel = upper.find("LIMIT").unwrap_or(0);
    let abs_start = start + rel;
    let abs_end = abs_start + 5;
    out.push(Diagnostic {
      code: "sql051",
      severity: Severity::Warning,
      message: "LIMIT without ORDER BY -- row selection is non-deterministic".into(),
      range: crate::range_at(abs_start, abs_end),
    });
  }
}
