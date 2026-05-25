//! sql285: `DROP ROLE foo` / `DROP USER foo` without a preceding
//! `REASSIGN OWNED BY foo` + `DROP OWNED BY foo`. PG raises 2BP01
//! when the role still owns any object (or has any privileges).
//! Hint: run the reassign/drop-owned pair first.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql285"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim_start();
    if !(trim.starts_with("DROP ROLE") || trim.starts_with("DROP USER") || trim.starts_with("DROP GROUP")) { return }
    let prelude_upper = source[..start].to_ascii_uppercase();
    if prelude_upper.contains("REASSIGN OWNED") && prelude_upper.contains("DROP OWNED") { return }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = start + body.find(';').unwrap_or(body.len());
    out.push(Diagnostic {
      code: "sql285",
      severity: Severity::Hint,
      message: "DROP ROLE/USER without preceding REASSIGN OWNED + DROP OWNED -- fails when role owns any object (PG 2BP01); run the reassign pair first".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
