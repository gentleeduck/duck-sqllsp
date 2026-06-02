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
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_comments_only(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("ALTER TABLE") {
      return;
    }
    let needle = if let Some(at) = upper.find("DISABLE TRIGGER") {
      (at, "DISABLE TRIGGER")
    } else if let Some(at) = upper.find("ENABLE TRIGGER") {
      (at, "ENABLE TRIGGER")
    } else if let Some(at) = upper.find("ENABLE ALWAYS TRIGGER") {
      (at, "ENABLE ALWAYS TRIGGER")
    } else if let Some(at) = upper.find("ENABLE REPLICA TRIGGER") {
      (at, "ENABLE REPLICA TRIGGER")
    } else {
      return;
    };
    let abs_s = start + needle.0;
    let abs_e = abs_s + needle.1.len();
    out.push(Diagnostic {
      code: "sql347",
      severity: Severity::Hint,
      message: format!(
        "{} takes ACCESS EXCLUSIVE on the table -- blocks readers + writers; run during low traffic or set lock_timeout",
        needle.1
      ),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
