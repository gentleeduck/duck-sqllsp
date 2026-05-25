//! sql347: `ALTER TABLE t ENABLE|DISABLE TRIGGER ...`. Takes an
//! ACCESS EXCLUSIVE lock on the target table, which blocks every read
//! AND every write until the catalog mutation commits. Hint about
//! running during low traffic or wrapping in `lock_timeout`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql347"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.starts_with("ALTER TABLE") && !upper.contains("ALTER TABLE") { return }
    let needle = if let Some(at) = upper.find("DISABLE TRIGGER") { (at, "DISABLE TRIGGER") }
      else if let Some(at) = upper.find("ENABLE TRIGGER") { (at, "ENABLE TRIGGER") }
      else if let Some(at) = upper.find("ENABLE ALWAYS TRIGGER") { (at, "ENABLE ALWAYS TRIGGER") }
      else if let Some(at) = upper.find("ENABLE REPLICA TRIGGER") { (at, "ENABLE REPLICA TRIGGER") }
      else { return };
    let abs_s = start + needle.0;
    let abs_e = abs_s + needle.1.len();
    out.push(Diagnostic {
      code: "sql347",
      severity: Severity::Hint,
      message: format!(
        "{} takes ACCESS EXCLUSIVE on the table -- blocks readers + writers; run during low traffic or set lock_timeout",
        needle.1
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
