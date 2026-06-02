//! sql251: `SELECT * FROM t ORDER BY 1` -- positional ORDER BY
//! on a `*` projection is brittle: adding or reordering columns
//! changes which column the sort happens on. Hint: name the column.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql251"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(s) = &stmt.kind else { return };
    if !s.projections.iter().any(|p| matches!(p, dsl_parse::Projection::Star)) {
      return;
    }
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    let Some(at) = upper.find("ORDER BY ") else { return };
    let after = at + "ORDER BY ".len();
    let rest = &body[after..];
    let id_end = rest.find(|c: char| c == ',' || c == ';' || c == '\n' || c.is_whitespace()).unwrap_or(rest.len());
    let first = rest[..id_end].trim();
    if first.parse::<u32>().is_err() {
      return;
    }
    let abs_s = start + at;
    let abs_e = start + after + id_end;
    out.push(Diagnostic {
      code: "sql251",
      severity: Severity::Hint,
      message: format!(
        "ORDER BY {first} on a `SELECT *` projection -- positional sort is brittle when columns change; name the column"
      ),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
