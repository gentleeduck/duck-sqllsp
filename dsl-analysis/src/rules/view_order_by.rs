//! sql577: `CREATE VIEW v AS SELECT ... ORDER BY x` -- an ORDER BY in a
//! (non-materialized) view definition is not guaranteed to survive: when you
//! `SELECT ... FROM v` the planner is free to re-order, so the sort is wasted
//! work and a false promise. Sort in the queries that read the view instead.
//! (ORDER BY ... LIMIT is a deliberate top-N and is left alone; MATERIALIZED
//! views, where the order is materialized once, are also skipped.)

use crate::clause_scan::{find_clause, find_clause_end};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql577"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("CREATE") || !upper.contains("VIEW") || !upper.contains(" AS ") {
      return;
    }
    if upper.contains("MATERIALIZED VIEW") {
      return;
    }
    let ub = upper.as_bytes();
    // Top-level ORDER BY of the view's SELECT.
    let Some(at) = find_clause(ub, b"ORDER") else { return };
    // Skip when paired with LIMIT/OFFSET/FETCH (deliberate top-N).
    let end = find_clause_end(ub, at + 5, &["LIMIT", "OFFSET", "FETCH"]);
    if end < ub.len() {
      // find_clause_end stopped on a boundary keyword; if that boundary is a
      // LIMIT/OFFSET/FETCH (not `)` / `;`), the ORDER BY is meaningful.
      let b = ub[end];
      if b != b')' && b != b';' {
        return;
      }
    }
    out.push(Diagnostic {
      code: "sql577",
      severity: Severity::Hint,
      message: "ORDER BY in a view definition isn't preserved when the view is queried -- sort in the reading query".into(),
      range: crate::range_at(start + at, start + at + 5),
    });
  }
}
