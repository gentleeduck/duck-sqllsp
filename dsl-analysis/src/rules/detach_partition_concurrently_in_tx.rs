//! sql761: `ALTER TABLE ... DETACH PARTITION ... CONCURRENTLY` inside
//! an explicit transaction. Like `DROP INDEX CONCURRENTLY` (sql331),
//! the CONCURRENTLY detach variant cannot run inside a BEGIN/COMMIT
//! block -- PG raises 25001 at runtime. Flags when the same buffer
//! mixes a CONCURRENTLY detach with an earlier BEGIN.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql761"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(rel) = upper.find("DETACH PARTITION") else { return };
    if !upper[rel..].contains("CONCURRENTLY") {
      return;
    }
    let prefix_upper = source[..start].to_ascii_uppercase();
    if !prefix_upper.contains("BEGIN") && !prefix_upper.contains("START TRANSACTION") {
      return;
    }
    let abs_s = start + rel;
    out.push(Diagnostic {
      code: "sql761",
      severity: Severity::Error,
      message:
        "DETACH PARTITION ... CONCURRENTLY cannot run inside a transaction (25001) -- move it out of BEGIN/COMMIT"
          .into(),
      range: crate::range_at(abs_s, abs_s + "DETACH PARTITION".len()),
    });
  }
}
