//! sql136: `COPY t FROM 'file'` without a `FORMAT` clause -- defaults
//! to `text` which has subtle escaping rules. Hint to make the format
//! explicit.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql136"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("COPY ") {
      return;
    }
    // If FORMAT or CSV/TEXT/BINARY appears anywhere in the stmt, skip.
    if upper.contains("FORMAT") || upper.contains(" CSV") || upper.contains(" TEXT ") || upper.contains(" BINARY") {
      return;
    }
    let leading = upper.len() - trimmed.len();
    let abs_start = start + leading;
    let abs_end = abs_start + 4;
    out.push(Diagnostic {
            code: "sql136",
            severity: Severity::Hint,
            message: "COPY without an explicit FORMAT clause defaults to `text` -- spell it (`WITH (FORMAT csv, ...)`) so the file shape is unambiguous".into(),
            range: crate::range_at(abs_start, abs_end),
        });
  }
}
