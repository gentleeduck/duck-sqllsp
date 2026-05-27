//! sql501: `ORDER BY not_null_col NULLS FIRST|LAST` -- the NULLS
//! clause is redundant because the column can never be NULL. Drop
//! the `NULLS ...` to make the intent (and the query plan) cleaner.

use crate::clause_scan::{find_clause, find_clause_end, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql501"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    // Resolve a single target table -- needed for catalog nullability lookup.
    let Some(target) = (match &stmt.kind {
      StatementKind::Select(s) => {
        if s.from.len() != 1 {
          return;
        }
        s.from.first()
      },
      _ => return,
    }) else {
      return;
    };
    let Some(t) = catalog.find_table(target.schema.as_deref(), &target.name) else { return };

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
      let trimmed = item.trim();
      if trimmed.is_empty() {
        continue;
      }
      // Look for `NULLS FIRST` or `NULLS LAST` at the end of the
      // item, then read back over an optional ASC/DESC, then read
      // the column identifier.
      let upper_item = trimmed.to_ascii_uppercase();
      let nulls_kind = if upper_item.ends_with(" NULLS LAST") {
        "LAST"
      } else if upper_item.ends_with(" NULLS FIRST") {
        "FIRST"
      } else {
        continue;
      };
      let tail_len = if nulls_kind == "LAST" { " NULLS LAST".len() } else { " NULLS FIRST".len() };
      let before_nulls = trimmed[..trimmed.len() - tail_len].trim_end();
      // Strip an optional trailing ASC/DESC.
      let core = {
        let upper_before = before_nulls.to_ascii_uppercase();
        if upper_before.ends_with(" ASC") {
          before_nulls[..before_nulls.len() - " ASC".len()].trim_end()
        } else if upper_before.ends_with(" DESC") {
          before_nulls[..before_nulls.len() - " DESC".len()].trim_end()
        } else {
          before_nulls
        }
      };
      let Some((qualifier, name)) = parse_simple_ident(core) else {
        continue;
      };
      // Qualifier must match table alias or name when present.
      if let Some(q) = &qualifier {
        let q_lc = q.to_ascii_lowercase();
        let alias_match = target.alias.as_deref().map(str::to_ascii_lowercase).as_deref() == Some(q_lc.as_str());
        let table_match = q_lc == target.name.to_ascii_lowercase();
        if !alias_match && !table_match {
          continue;
        }
      }
      let Some(col) = t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(&name)) else {
        continue;
      };
      if col.nullable {
        continue;
      }
      let item_abs_start = start + rel_clause_start + 8 + item_rel_off;
      let item_leading = item.len() - item.trim_start().len();
      let abs_s = item_abs_start + item_leading;
      let abs_e = abs_s + trimmed.len();
      out.push(Diagnostic {
        code: "sql501",
        severity: Severity::Hint,
        message: format!(
          "`NULLS {nulls_kind}` on `{}` is redundant -- column is NOT NULL, so no NULLs can appear in the sort. Drop the `NULLS {nulls_kind}` clause.",
          col.name
        ),
        range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
    }
  }
}
