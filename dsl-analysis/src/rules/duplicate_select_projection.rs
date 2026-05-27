//! sql410: `SELECT id, id FROM ...` -- a column appears twice in the
//! SELECT list. PG accepts this (the output has two identically-named
//! columns) but it's almost always a copy-paste typo. The duplicate
//! also breaks code that builds dicts/structs keyed by column name.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Expr, Projection, Statement, StatementKind};
use dsl_resolve::Scope;
use std::collections::HashSet;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql410"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, _source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut emitted: HashSet<String> = HashSet::new();
    for p in &s.projections {
      let Projection::Expr { expr, alias } = p else {
        continue;
      };
      // Effective output name: explicit alias wins; otherwise the
      // bare column name. Anything else (function calls, literals)
      // we skip -- the diagnostic is for accidental column repeats,
      // not for `SELECT count(*), count(*)` style duplicates.
      let key_owned: String = if let Some(a) = alias {
        a.to_ascii_lowercase()
      } else if let Expr::Column { qualifier, name, .. } = expr {
        let mut k = String::new();
        if let Some(q) = qualifier {
          k.push_str(&q.to_ascii_lowercase());
          k.push('.');
        }
        k.push_str(&name.to_ascii_lowercase());
        k
      } else {
        continue;
      };
      if !seen.insert(key_owned.clone()) && emitted.insert(key_owned.clone()) {
        out.push(Diagnostic {
          code: "sql410",
          severity: Severity::Warning,
          message: format!(
            "column `{key_owned}` is selected more than once -- the duplicate is allowed by PG but typically a typo"
          ),
          range: stmt.range,
        });
      }
    }
  }
}
