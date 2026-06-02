//! sql403: ORDER BY references a column that doesn't exist in any
//! in-scope table or projection alias.
//!
//! PG models ORDER BY as part of the SELECT but our AST doesn't expose
//! it -- so we text-scan inside the statement's source range. We only
//! flag bare `<ident>` or `<qualifier>.<ident>` items (skipping
//! expressions, function calls, positional `ORDER BY 1`, and items
//! that resolve to a projection alias) to keep this honest.

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
    "sql403"
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
    let Some(rel_clause_start) = find_clause(bytes, b"ORDER BY") else {
      return;
    };
    let clause_end = find_clause_end(bytes, rel_clause_start + 8, &["LIMIT", "OFFSET", "FOR", "HAVING", "FETCH", "WINDOW"]);
    let clause = &cleaned[rel_clause_start + 8..clause_end];
    let raw_clause = &raw[rel_clause_start + 8..clause_end];

    for (item, item_rel_off) in split_top_level(clause) {
      // If the RAW item begins with a string-literal quote or
      // dollar-quote, this sort key is a constant expression, not a
      // column reference -- strip_noise_full blanked the literal to
      // spaces, which would otherwise expose a trailing `DESC`/`ASC`
      // as a fake bare identifier. (sql433 handles the constant-sort
      // no-op itself.)
      let raw_item = &raw_clause[item_rel_off..item_rel_off + item.len()];
      if raw_item.trim_start().starts_with(['\'', '$']) {
        continue;
      }
      let trimmed = strip_order_modifiers(item.trim());
      if trimmed.is_empty() {
        continue;
      }
      if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        continue;
      }
      let Some((qualifier, name)) = parse_simple_ident(trimmed) else {
        continue;
      };
      // Keyword literals (NULL, TRUE, FALSE) and built-in
      // identifier-shaped expressions (CURRENT_DATE, CURRENT_USER,
      // SESSION_USER, etc.) parse as bare idents but are not column
      // references. Skip them. (`ORDER BY NULL` is now caught by
      // sql433 as a constant-sort no-op.)
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
        code: "sql403",
        severity: Severity::Error,
        message: format!("unknown column `{display}` in ORDER BY"),
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

fn strip_order_modifiers(s: &str) -> &str {
  let mut t = s.trim_end();
  let mut changed = true;
  while changed {
    changed = false;
    let upper = t.to_ascii_uppercase();
    for tail in [" NULLS FIRST", " NULLS LAST", " ASC", " DESC"] {
      if upper.ends_with(tail) {
        t = t[..t.len() - tail.len()].trim_end();
        changed = true;
        break;
      }
    }
  }
  t
}
