//! sql014: implicit cross join.
//!
//! `FROM a, b WHERE ...` without a join predicate between `a` and `b`
//! produces a Cartesian product. Usually a missing `ON` clause.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Expr, Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql014"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
    if s.from.len() <= 1 {
      return;
    }
    // Skip when any FROM source is synthetic (function-call / subquery
    // / CTE alias) -- those bind via outer-side correlation (LATERAL)
    // or via subquery scope; the rule's "join must have qualifier on
    // each table" heuristic doesn't fit.
    if s.from.iter().any(|t| t.schema.as_deref().map_or(false, |sc| sc.starts_with('<'))) {
      return;
    }
    // Skip LATERAL forms explicitly -- they're correlated joins, not
    // cross joins.
    let stmt_start: usize = u32::from(stmt.range.start()) as usize;
    let stmt_end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let stmt_upper_for_lateral = source[stmt_start..stmt_end].to_ascii_uppercase();
    if stmt_upper_for_lateral.contains("LATERAL") {
      return;
    }

    // Collect FROM-side table names + aliases.
    let names: Vec<String> = s
      .from
      .iter()
      .flat_map(|t| {
        let mut v = vec![t.name.clone()];
        if let Some(a) = &t.alias {
          v.push(a.clone());
        }
        v
      })
      .collect();

    // Walk WHERE expression for any qualifier that names two
    // different FROM tables in the same predicate. Heuristic: if at
    // least one column ref is qualified with each table name, treat
    // as a join.
    let mut qualifiers: Vec<String> = Vec::new();
    if let Some(w) = &s.where_clause {
      collect_qualifiers(w, &mut qualifiers);
    }

    let mut covered = 0;
    for t in &s.from {
      let alias = t.alias.clone().unwrap_or_else(|| t.name.clone());
      if qualifiers.iter().any(|q| q == &alias || q == &t.name) {
        covered += 1;
      }
    }

    if covered < s.from.len() {
      let _ = names; // kept for future richer message
      // Narrow to the FROM keyword in the source.
      let stmt_start: usize = u32::from(stmt.range.start()) as usize;
      let stmt_end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
      let upper = source[stmt_start..stmt_end].to_ascii_uppercase();
      let range = upper
        .find(" FROM ")
        .map(|r| {
          let abs_start = stmt_start + r + 1;
          let abs_end = abs_start + 4;
          text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into())
        })
        .unwrap_or(stmt.range);
      out.push(Diagnostic {
        code: "sql014",
        severity: Severity::Warning,
        message: format!(
          "implicit cross join: {} tables in FROM but no predicate joins them all -- use explicit JOIN ... ON",
          s.from.len()
        ),
        range,
      });
    }
  }
}

fn collect_qualifiers(e: &Expr, out: &mut Vec<String>) {
  match e {
    Expr::Column { qualifier: Some(q), .. } => out.push(q.clone()),
    Expr::Column { .. } => {},
    Expr::BinaryOp { left, right, .. } => {
      collect_qualifiers(left, out);
      collect_qualifiers(right, out);
    },
    Expr::Call { args, .. } => {
      for a in args {
        collect_qualifiers(a, out);
      }
    },
    _ => {},
  }
}
