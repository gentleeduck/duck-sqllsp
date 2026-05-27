//! sql254: `ALTER TABLE t SET TABLESPACE ts` rewrites the entire
//! table on disk and holds AccessExclusiveLock for the duration.
//! On large tables this is a sustained outage. Hint: use ALTER
//! TABLE ... SET TABLESPACE ... NOWAIT or schedule a maintenance
//! window.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql254"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let raw = &source[start..end];
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("ALTER TABLE") {
      return;
    }
    let Some(at) = upper.find("SET TABLESPACE") else { return };
    let abs_s = start + at;
    let abs_e = abs_s + "SET TABLESPACE".len();
    out.push(Diagnostic {
      code: "sql254",
      severity: Severity::Warning,
      message: "ALTER TABLE SET TABLESPACE rewrites the table and holds AccessExclusiveLock -- schedule maintenance window or use pg_repack".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
