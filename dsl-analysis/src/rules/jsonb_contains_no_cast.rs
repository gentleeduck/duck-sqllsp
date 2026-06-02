//! sql232: `<jsonb col> @> 'foo'` (or `<@`) where the RHS is a plain
//! text literal without `::jsonb`. PG implicitly casts the literal
//! at runtime; the explicit `::jsonb` cast nudges the planner and
//! reads better. Hint, not error.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql232"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    for op in ["@>", "<@"] {
      let mut from = 0usize;
      while let Some(rel) = body[from..].find(op) {
        let at = from + rel;
        let after = at + op.len();
        let rest = body[after..].trim_start();
        // Only fire when RHS is a bare quoted literal without ::jsonb.
        if !rest.starts_with('\'') {
          from = after;
          continue;
        }
        let Some(close_rel) = rest[1..].find('\'') else { break };
        let after_lit = 1 + close_rel + 1;
        let post = rest[after_lit..].trim_start();
        if post.starts_with("::") {
          from = after;
          continue;
        }
        let lit_abs_s = start + after + (body[after..].len() - rest.len());
        let lit_abs_e = lit_abs_s + after_lit;
        out.push(Diagnostic {
          code: "sql232",
          severity: Severity::Hint,
          message: format!(
            "`{op}` text literal -- PG implicitly casts; add `::jsonb` for clarity + planner determinism"
          ),
          range: crate::range_at(lit_abs_s, lit_abs_e),
        });
        from = after + after_lit;
      }
    }
  }
}
