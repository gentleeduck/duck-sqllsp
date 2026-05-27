//! sql457: `SELECT a, b FROM t GROUP BY 3` -- the positional
//! reference points past the projection list. PG raises 42703
//! "GROUP BY position N is not in select list" at parse. Same for
//! `ORDER BY 5` when only 2 projections exist, and for
//! `GROUP BY 0` (positions are 1-based).

use crate::clause_scan::{find_clause, find_clause_end, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;
use text_size::TextRange;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql457"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
    let n_proj = s.projections.len();
    if n_proj == 0 {
      return;
    }
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let cleaned = crate::textutil::strip_noise_full(raw);
    let upper = cleaned.to_ascii_uppercase();
    let ub = upper.as_bytes();
    let stopwords = ["LIMIT", "OFFSET", "FOR", "HAVING", "FETCH", "WINDOW"];

    for (clause_kw, label) in [(&b"GROUP BY"[..], "GROUP BY"), (&b"ORDER BY"[..], "ORDER BY")] {
      let Some(rel) = find_clause(ub, clause_kw) else { continue };
      let body_start = rel + clause_kw.len();
      // ORDER BY can follow GROUP BY; GROUP BY ends at ORDER BY too.
      let mut extra_stops: Vec<&str> = stopwords.to_vec();
      if label == "GROUP BY" {
        extra_stops.push("ORDER BY");
      }
      let body_end = find_clause_end(ub, body_start, &extra_stops);
      let body = &cleaned[body_start..body_end];
      for (item, item_rel_off) in split_top_level(body) {
        let trimmed = strip_modifiers(item.trim());
        let Ok(pos) = trimmed.parse::<i64>() else { continue };
        if pos < 1 || (pos as usize) > n_proj {
          let abs_s = start + body_start + item_rel_off;
          let abs_e = abs_s + item.trim_end().len();
          out.push(Diagnostic {
            code: "sql457",
            severity: Severity::Error,
            message: format!(
              "{label} position `{pos}` is out of range -- the projection list has {n_proj} item{} (positions are 1-based). PG raises 42703 \"{label} position {pos} is not in select list\"",
              if n_proj == 1 { "" } else { "s" }
            ),
            range: TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
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
