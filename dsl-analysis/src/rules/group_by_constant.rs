//! sql480: `GROUP BY NULL` / `GROUP BY TRUE` / `GROUP BY 'foo'` --
//! grouping by a constant collapses every row into a single bucket,
//! which is semantically equivalent to having no GROUP BY at all (when
//! the projection is purely aggregates). Almost always a leftover or
//! a mistaken attempt to group by a column whose name was typed as a
//! string literal. Counterpart to sql433 for the GROUP BY clause.
//!
//! Positional `GROUP BY 1` is a real column reference (1st projection)
//! and is *not* flagged here -- that's sql065's territory.

use crate::clause_scan::{find_clause, find_clause_end, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql480"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
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

    for (item, item_rel_off) in split_top_level(clause) {
      // Read from RAW source -- strip_noise_full blanks string
      // literals so `'foo'` would look empty in `cleaned`.
      let item_abs_start = start + rel_clause_start + 8 + item_rel_off;
      let item_abs_end = (item_abs_start + item.len()).min(source.len());
      let raw_item = &source[item_abs_start..item_abs_end];
      let raw_trimmed = raw_item.trim();
      if raw_trimmed.is_empty() {
        continue;
      }
      let Some(kind) = classify_constant(raw_trimmed) else {
        continue;
      };
      let leading_ws = raw_item.len() - raw_item.trim_start().len();
      let abs_start = item_abs_start + leading_ws;
      let abs_end = abs_start + raw_trimmed.len();
      out.push(Diagnostic {
        code: "sql480",
        severity: Severity::Warning,
        message: format!(
          "GROUP BY {kind} collapses every row into a single bucket -- this is equivalent to having no GROUP BY at all. If you meant to group by a column, drop the quotes; if not, remove the GROUP BY clause."
        ),
        range: TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
      });
    }
  }
}

fn classify_constant(s: &str) -> Option<&'static str> {
  let u = s.to_ascii_uppercase();
  if u == "NULL" {
    return Some("NULL");
  }
  if u == "TRUE" || u == "FALSE" {
    return Some("a boolean constant");
  }
  if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
    return Some("a string literal");
  }
  None
}
