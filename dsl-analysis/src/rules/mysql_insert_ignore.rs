//! sql666: `INSERT IGNORE INTO ...` -- MySQL's modifier that silently skips
//! rows which would violate a unique/PK constraint (and downgrades other errors
//! to warnings). PostgreSQL has no `INSERT IGNORE`; express the intent
//! explicitly with `INSERT ... ON CONFLICT DO NOTHING`, which skips only
//! conflicting rows and still raises real errors.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql666"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let u = upper.trim_start();
    if !u.starts_with("INSERT") {
      return;
    }
    let b = upper.as_bytes();
    let lead = upper.len() - u.len();
    let mut j = lead + 6;
    let n = b.len();
    while j < n && b[j].is_ascii_whitespace() {
      j += 1;
    }
    if j + 6 <= n && &b[j..j + 6] == b"IGNORE" && b.get(j + 6).is_none_or(|&c| !is_word(c as char)) {
      out.push(Diagnostic {
        code: "sql666",
        severity: Severity::Error,
        message: "`INSERT IGNORE` is MySQL -- PostgreSQL uses `INSERT ... ON CONFLICT DO NOTHING` to skip conflicting rows".into(),
        range: crate::range_at(start + j, start + j + 6),
      });
    }
  }
}
