//! sql249: `INSERT INTO t DEFAULT VALUES` -- requires every column
//! to be NOT NULL with a DEFAULT, GENERATED, or nullable. Catches
//! the common case where the catalog table has a NOT NULL column
//! without DEFAULT (and not a serial / generated identity), which
//! PG raises 23502 at runtime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql249"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Insert(ins) = &stmt.kind else { return };
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("DEFAULT VALUES") { return }
    let Some(t) = catalog.find_table(ins.table.schema.as_deref(), &ins.table.name) else { return };
    let bad: Vec<&str> = t
      .columns
      .iter()
      .filter(|c| !c.nullable && c.default.is_none() && c.generated.is_none())
      .map(|c| c.name.as_str())
      .collect();
    if bad.is_empty() { return }
    let abs_s = start;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql249",
      severity: Severity::Error,
      message: format!(
        "INSERT DEFAULT VALUES into `{}` -- NOT NULL columns without DEFAULT: {} -- PG raises 23502",
        t.name,
        bad.join(", "),
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
