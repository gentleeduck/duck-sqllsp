//! sql232: `<jsonb col> @> 'foo'` (or `<@`, `?`, `?|`, `?&`) where
//! the RHS is a plain text literal without `::jsonb`. PG raises
//! 42883 "operator does not exist: jsonb @> text" at runtime.
//! Suggest the explicit ::jsonb cast.

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
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    for op in ["@>", "<@"] {
      let mut from = 0usize;
      while let Some(rel) = body[from..].find(op) {
        let at = from + rel;
        let after = at + op.len();
        let rest = body[after..].trim_start();
        // Only fire when RHS is a bare quoted literal without ::jsonb.
        if !rest.starts_with('\'') { from = after; continue }
        let Some(close_rel) = rest[1..].find('\'') else { from = after; break };
        let after_lit = 1 + close_rel + 1;
        let post = rest[after_lit..].trim_start();
        if post.starts_with("::") { from = after; continue }
        let lit_abs_s = start + after + (body[after..].len() - rest.len());
        let lit_abs_e = lit_abs_s + after_lit;
        out.push(Diagnostic {
          code: "sql232",
          severity: Severity::Error,
          message: format!(
            "`{op}` against text literal -- LHS is jsonb; cast RHS with `::jsonb` (PG 42883)"
          ),
          range: text_size::TextRange::new((lit_abs_s as u32).into(), (lit_abs_e as u32).into()),
        });
        from = after + after_lit;
      }
    }
  }
}
