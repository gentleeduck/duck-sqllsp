//! sql426: `SELECT DISTINCT id FROM users ORDER BY age` -- PG raises
//! "for SELECT DISTINCT, ORDER BY expressions must appear in select
//! list". The DISTINCT deduplicates the projection, so any sort key
//! must be derivable from those columns. Add the column to the
//! projection or drop the DISTINCT.

use crate::clause_scan::{find_clause, find_clause_end, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Projection, Statement, StatementKind};
use dsl_resolve::Scope;
use std::collections::HashSet;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql426"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
    // `SELECT *` covers everything -- skip the rule.
    if s.projections.iter().any(|p| matches!(p, Projection::Star)) {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    // Require `SELECT DISTINCT` (not DISTINCT ON).
    let Some(sel_at) = find_clause(ub, b"SELECT") else {
      return;
    };
    let Some(dist_at) = find_clause(&ub[sel_at + 6..], b"DISTINCT").map(|p| p + sel_at + 6) else {
      return;
    };
    // Check it's not DISTINCT ON.
    let after_dist = &cleaned[dist_at + 8..];
    let upper_after = after_dist.to_ascii_uppercase();
    if upper_after.trim_start().starts_with("ON ") || upper_after.trim_start().starts_with("ON(") {
      return;
    }
    // ORDER BY clause body.
    let Some(ob_at) = find_clause(ub, b"ORDER BY") else {
      return;
    };
    let ob_end = find_clause_end(ub, ob_at + 8, &["LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW"]);
    let order_body = &cleaned[ob_at + 8..ob_end];

    // Collect projection identifiers (effective output names).
    let mut proj_set: HashSet<String> = HashSet::new();
    for p in &s.projections {
      if let Projection::Expr { expr, alias } = p {
        if let Some(a) = alias {
          proj_set.insert(a.to_ascii_lowercase());
        }
        if let dsl_parse::Expr::Column { name, .. } = expr {
          proj_set.insert(name.to_ascii_lowercase());
        }
      }
    }
    if proj_set.is_empty() {
      return;
    }

    for (item, _off) in split_top_level(order_body) {
      let trimmed = strip_order_modifiers(item.trim());
      if trimmed.is_empty() {
        continue;
      }
      if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        continue; // positional ORDER BY
      }
      let Some((_qual, name)) = parse_simple_ident(trimmed) else {
        continue; // expression ORDER BY -- harder; skip for safety
      };
      if !proj_set.contains(&name.to_ascii_lowercase()) {
        out.push(Diagnostic {
          code: "sql426",
          severity: Severity::Error,
          message: format!(
            "ORDER BY `{name}` is not in SELECT DISTINCT projection -- PG rejects this; add `{name}` to the SELECT list or drop DISTINCT"
          ),
          range: stmt.range,
        });
      }
    }
  }
}

fn strip_order_modifiers(s: &str) -> &str {
  let mut t = s.trim_end();
  let mut changed = true;
  while changed {
    changed = false;
    let up = t.to_ascii_uppercase();
    for tail in [" NULLS FIRST", " NULLS LAST", " ASC", " DESC"] {
      if up.ends_with(tail) {
        t = t[..t.len() - tail.len()].trim_end();
        changed = true;
        break;
      }
    }
  }
  t
}
