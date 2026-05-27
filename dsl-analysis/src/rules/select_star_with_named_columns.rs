//! sql430: `SELECT *, col FROM t` -- mixing `*` with an explicit
//! column name duplicates that column in the output (PG returns
//! every column AND `col`). Almost always a typo or a stray paste;
//! either drop the `*` or drop the named column.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Projection, Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql430"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, _source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(s) = &stmt.kind else {
      return;
    };
    let has_star = s.projections.iter().any(|p| matches!(p, Projection::Star));
    if !has_star || s.projections.len() <= 1 {
      return;
    }
    // Only flag when at least one OTHER projection is a bare named
    // column (not a function call, expression). Mixing `*` with an
    // aliased expression (`SELECT *, count(*) OVER ()`) is a real
    // pattern; don't flag those.
    let has_named_col = s.projections.iter().any(|p| matches!(p, Projection::Expr { expr: dsl_parse::Expr::Column { .. }, .. }));
    if !has_named_col {
      return;
    }
    out.push(Diagnostic {
      code: "sql430",
      severity: Severity::Warning,
      message: "`SELECT *, col` mixes the star projection with an explicit column -- the column appears TWICE in the output; drop one".into(),
      range: stmt.range,
    });
  }
}
