//! sql412: `ORDER BY id, id` / `GROUP BY id, id` -- a column appears
//! more than once in the clause. The repeat does nothing (ORDER BY is
//! already deterministic on the first occurrence; GROUP BY repeats are
//! redundant), and is almost always a typo for two different columns.
//!
//! Distinct directions (`ORDER BY id ASC, id DESC`) are still flagged
//! because the second sort key is unreachable -- the first ordering
//! already pins every row.

use crate::clause_scan::{find_clause, find_clause_end, parse_simple_ident, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use std::collections::HashSet;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql412"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(_) = &stmt.kind else {
      return;
    };
    let (_start, raw) = crate::stmt_body(stmt, source);
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let bytes_u = upper.as_bytes();

    for (needle, label) in [(&b"ORDER BY"[..], "ORDER BY"), (&b"GROUP BY"[..], "GROUP BY")] {
      let Some(rel_start) = find_clause(bytes_u, needle) else {
        continue;
      };
      let stopwords: &[&str] = if needle == b"ORDER BY" {
        &["LIMIT", "OFFSET", "FOR", "HAVING", "FETCH", "WINDOW"]
      } else {
        &["HAVING", "ORDER BY", "LIMIT", "OFFSET", "FOR", "FETCH", "WINDOW"]
      };
      let clause_end = find_clause_end(bytes_u, rel_start + needle.len(), stopwords);
      let clause = &cleaned[rel_start + needle.len()..clause_end];

      let mut seen: HashSet<String> = HashSet::new();
      let mut emitted: HashSet<String> = HashSet::new();
      for (item, _off) in split_top_level(clause) {
        let trimmed = strip_modifiers(item.trim());
        if trimmed.is_empty() {
          continue;
        }
        // Skip positional refs (sql099 / sql100 cover those).
        if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
          continue;
        }
        let Some((qual, name)) = parse_simple_ident(trimmed) else {
          continue;
        };
        let key = match &qual {
          Some(q) => format!("{}.{}", q.to_ascii_lowercase(), name.to_ascii_lowercase()),
          None => name.to_ascii_lowercase(),
        };
        if !seen.insert(key.clone()) && emitted.insert(key.clone()) {
          let display = qual.as_ref().map(|q| format!("{q}.{name}")).unwrap_or(name.clone());
          out.push(Diagnostic {
            code: "sql412",
            severity: Severity::Hint,
            message: format!("`{display}` appears twice in {label} -- the repeat has no effect"),
            range: stmt.range,
          });
        }
      }
    }
  }
}

fn strip_modifiers(s: &str) -> &str {
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
