//! sql404: GROUP BY references a column that doesn't exist.
//!
//! Mirrors [`order_by_unknown_column`](super::order_by_unknown_column)
//! but bounded by HAVING / ORDER BY / LIMIT / OFFSET / FOR / FETCH /
//! WINDOW. Projection aliases are accepted (PG allows them and we have
//! a separate stylistic rule, sql337, for the portability concern).
//! Items wrapped in ROLLUP/CUBE/GROUPING SETS or `(a, b)` grouping
//! expressions fall through naturally because parse_simple_ident
//! rejects anything with parens or commas inside an item.

use crate::clause_scan::{find_clause, find_clause_end, parse_simple_ident, split_top_level};
use crate::rules::unknown_column::column_exists;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Projection, Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql404"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if scope.is_empty() || catalog.tables().next().is_none() {
      return;
    }
    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
    let aliases: Vec<String> = s
      .projections
      .iter()
      .filter_map(|p| match p {
        Projection::Expr { alias: Some(a), .. } => Some(a.to_ascii_lowercase()),
        _ => None,
      })
      .collect();

    let (start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    let Some(rel_clause_start) = find_clause(bytes, b"GROUP BY") else {
      return;
    };
    let clause_end = find_clause_end(
      bytes,
      rel_clause_start + 8,
      &["HAVING", "ORDER BY", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW"],
    );
    let clause = &cleaned[rel_clause_start + 8..clause_end];
    let raw_clause = &raw[rel_clause_start + 8..clause_end];

    for (item, item_rel_off) in split_top_level(clause) {
      // Skip items whose RAW form starts with `'` or `$` -- the
      // cleaned text has the literal blanked to spaces, which would
      // otherwise leave us with empty trim and silently misfire.
      let raw_item = &raw_clause[item_rel_off..item_rel_off + item.len()];
      if raw_item.trim_start().starts_with(['\'', '$']) {
        continue;
      }
      let trimmed = item.trim();
      if trimmed.is_empty() {
        continue;
      }
      // skip positional refs (`GROUP BY 1`) -- sql100-family covers them
      if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        continue;
      }
      // skip ROLLUP/CUBE/GROUPING wrappers (parse_simple_ident will
      // reject these because of the trailing `(`, but bail early so we
      // skip the alias/known-column work too).
      let upper_item = trimmed.to_ascii_uppercase();
      if upper_item.starts_with("ROLLUP(")
        || upper_item.starts_with("CUBE(")
        || upper_item.starts_with("GROUPING SETS")
        || upper_item.starts_with('(')
      {
        continue;
      }
      let Some((qualifier, name)) = parse_simple_ident(trimmed) else {
        continue;
      };
      // Keyword literals (NULL, TRUE, FALSE) and built-in
      // identifier-shaped expressions (CURRENT_DATE, SESSION_USER,
      // etc.) parse as bare idents but are not column references.
      // sql480 handles the constant-grouping no-op for these.
      if qualifier.is_none() && is_keyword_literal(&name) {
        continue;
      }
      if qualifier.is_none() && aliases.iter().any(|a| a == &name.to_ascii_lowercase()) {
        continue;
      }
      if column_exists(scope, catalog, qualifier.as_deref(), &name) {
        continue;
      }
      let display = match &qualifier {
        Some(q) => format!("{q}.{name}"),
        None => name.clone(),
      };
      let abs_start = start + rel_clause_start + 8 + item_rel_off;
      let abs_end = abs_start + item.trim_end().len();
      out.push(Diagnostic {
        code: "sql404",
        severity: Severity::Error,
        message: format!("unknown column `{display}` in GROUP BY"),
        range: TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
      });
    }
  }
}

fn is_keyword_literal(name: &str) -> bool {
  matches!(
    name.to_ascii_uppercase().as_str(),
    "NULL"
      | "TRUE"
      | "FALSE"
      | "CURRENT_DATE"
      | "CURRENT_TIME"
      | "CURRENT_TIMESTAMP"
      | "CURRENT_USER"
      | "CURRENT_ROLE"
      | "CURRENT_CATALOG"
      | "CURRENT_SCHEMA"
      | "SESSION_USER"
      | "USER"
      | "LOCALTIME"
      | "LOCALTIMESTAMP"
  )
}
