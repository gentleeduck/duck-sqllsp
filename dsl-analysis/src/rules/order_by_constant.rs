//! sql433: `ORDER BY NULL` / `ORDER BY TRUE` / `ORDER BY 'foo'` --
//! sorting by a constant is a no-op in PG (every row gets the same
//! sort key). Almost always a MySQL idiom: MySQL used `ORDER BY NULL`
//! to suppress the implicit sort GROUP BY imposed. PG has no implicit
//! sort, so the clause is dead. Either drop it or sort by a real
//! column.
//!
//! Positional `ORDER BY 1` is a real column reference (1st projection)
//! and is *not* flagged here -- that's sql099's territory.

use crate::clause_scan::{find_clause, find_clause_end, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql433"
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
    let Some(rel_clause_start) = find_clause(bytes, b"ORDER BY") else {
      return;
    };
    let clause_end = find_clause_end(bytes, rel_clause_start + 8, &["LIMIT", "OFFSET", "FOR", "HAVING", "FETCH", "WINDOW"]);
    let clause = &cleaned[rel_clause_start + 8..clause_end];

    for (item, item_rel_off) in split_top_level(clause) {
      // Use the RAW source for trim + classification -- strip_noise_full
      // blanks out string literals AND their quotes in `cleaned`, so
      // `'foo'` becomes a run of spaces.
      let item_abs_start = start + rel_clause_start + 8 + item_rel_off;
      let item_abs_end = (item_abs_start + item.len()).min(source.len());
      let raw_item = &source[item_abs_start..item_abs_end];
      let raw_trimmed = strip_order_modifiers(raw_item.trim());
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
        code: "sql433",
        severity: Severity::Warning,
        message: format!(
          "ORDER BY {kind} is a no-op -- every row gets the same sort key, so the clause has no effect (MySQL used `ORDER BY NULL` to suppress GROUP BY's implicit sort; PG has no such sort); drop the clause or order by a real column"
        ),
        range: TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
      });
    }
  }
}

/// Returns a short label if `s` is a constant ORDER BY item.
/// Positional integers (`ORDER BY 1`) are NOT classified as constants
/// -- those reference a projection by position and are sql099's job.
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
