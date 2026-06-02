//! sql128: `GRANT ... TO PUBLIC` -- grants the privilege to *every*
//! current and future role. Almost always a mistake.

use crate::{Diagnostic, LintRule, Severity};
use crate::textutil::is_word;
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql128"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("GRANT ") {
      return;
    }
    // Find `TO PUBLIC` (case-insensitive, word-bounded).
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i + 9 <= n {
      if &upper[i..i + 9] == "TO PUBLIC"
        && (i == 0 || !is_word(bytes[i - 1] as char))
        && (i + 9 == n || !is_word(bytes[i + 9] as char))
      {
        let abs_start = start + i;
        let abs_end = start + i + 9;
        out.push(Diagnostic {
          code: "sql128",
          severity: Severity::Warning,
          message: "GRANT TO PUBLIC opens the privilege to every role -- target a specific role or group instead"
            .into(),
          range: crate::range_at(abs_start, abs_end),
        });
        return;
      }
      i += 1;
    }
  }
}

