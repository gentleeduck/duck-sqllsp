//! sql418: `SELECT DISTINCT pk_col FROM t` -- DISTINCT is redundant
//! when the projection contains the columns of a PRIMARY KEY or
//! UNIQUE constraint (rows are already distinct on those columns).
//! Drop the DISTINCT to avoid the implicit sort PG performs to
//! deduplicate.

use crate::clause_scan::{find_clause, find_clause_end};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::{Catalog, ConstraintKind};
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql418"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
    // Single-table SELECT only (joins break per-table uniqueness).
    if s.from.len() != 1 || !s.joins.is_empty() {
      return;
    }
    let t = &s.from[0];
    let Some(table) = catalog.find_table(t.schema.as_deref().or(Some("public")), &t.name) else {
      return;
    };

    // Check for DISTINCT keyword in stmt body, between SELECT and FROM.
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    // Find SELECT (top-level).
    let Some(sel_at) = find_clause(ub, b"SELECT") else {
      return;
    };
    let Some(dist_at) = find_clause(&ub[sel_at + 6..], b"DISTINCT").map(|p| p + sel_at + 6) else {
      return;
    };
    // Make sure FROM exists after DISTINCT at top level.
    let from_at = match find_clause(&ub[dist_at + 8..], b"FROM").map(|p| p + dist_at + 8) {
      Some(x) => x,
      None => return,
    };
    // Also ensure no ON BETWEEN -- DISTINCT ON has different semantics.
    let after_dist = &cleaned[dist_at + 8..from_at];
    let upper_after = after_dist.to_ascii_uppercase();
    if upper_after.trim_start().starts_with("ON ") || upper_after.trim_start().starts_with("ON(") {
      return;
    }
    // Pull projection list and extract bare column names.
    let proj_items: Vec<String> = after_dist
      .split(',')
      .map(|s| s.trim().to_string())
      .filter(|s| !s.is_empty())
      .collect();
    let mut proj_cols: Vec<String> = Vec::new();
    for item in &proj_items {
      let bare = item.rsplit('.').next().unwrap_or(item);
      if bare.is_empty() || !bare.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return; // expression projection -- skip whole rule
      }
      proj_cols.push(bare.to_ascii_lowercase());
    }
    if proj_cols.is_empty() {
      return;
    }
    // Find a PK or UNIQUE whose columns are all in the projection.
    for con in &table.constraints {
      if !matches!(con.kind, ConstraintKind::PrimaryKey | ConstraintKind::Unique) {
        continue;
      }
      if con.columns.is_empty() {
        continue;
      }
      let covers = con.columns.iter().all(|c| proj_cols.iter().any(|p| p.eq_ignore_ascii_case(c)));
      if !covers {
        continue;
      }
      let kind = match con.kind {
        ConstraintKind::PrimaryKey => "PRIMARY KEY",
        ConstraintKind::Unique => "UNIQUE",
        _ => unreachable!(),
      };
      let cols = con.columns.join(", ");
      let abs_s = start + dist_at;
      let abs_e = abs_s + 8;
      out.push(Diagnostic {
        code: "sql418",
        severity: Severity::Hint,
        message: format!(
          "DISTINCT is redundant: projection covers {kind} (`{cols}`) of `{}`; rows are already distinct",
          t.name
        ),
        range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      return; // one diagnostic per statement
    }
    // Bound to find_clause_end for compiler-warning satisfaction.
    let _ = find_clause_end;
  }
}
