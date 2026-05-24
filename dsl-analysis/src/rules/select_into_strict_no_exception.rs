//! sql156: `SELECT ... INTO STRICT var` inside PL/pgSQL without a
//! surrounding EXCEPTION block. STRICT raises NO_DATA_FOUND or
//! TOO_MANY_ROWS on miss -- an uncaught raise aborts the transaction.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql156"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("LANGUAGE PLPGSQL") && !upper.contains("DO $$") {
      return;
    }
    // Find `INTO STRICT` and check whether the body contains an
    // EXCEPTION block.
    let Some(open) = body.find("$$") else { return };
    let Some(close_rel) = body[open + 2..].find("$$") else { return };
    let body_text = &body[open + 2..open + 2 + close_rel];
    let body_up = body_text.to_ascii_uppercase();
    let Some(rel) = body_up.find("INTO STRICT") else { return };
    // Has EXCEPTION block? Either `BEGIN ... EXCEPTION` or the
    // ANSI-ish `EXCEPTION WHEN ...`.
    if body_up.contains("EXCEPTION WHEN") || body_up.contains("EXCEPTION\nWHEN") {
      return;
    }
    // Standalone EXCEPTION keyword (allowing whitespace before WHEN).
    if body_up.split("EXCEPTION").skip(1).any(|s| s.trim_start().starts_with("WHEN")) {
      return;
    }
    let abs_start = start + open + 2 + rel;
    let abs_end = abs_start + 11;
    out.push(Diagnostic {
      code: "sql156",
      severity: Severity::Hint,
      message:
        "SELECT INTO STRICT without surrounding EXCEPTION WHEN -- NO_DATA_FOUND / TOO_MANY_ROWS aborts the transaction"
          .into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}
