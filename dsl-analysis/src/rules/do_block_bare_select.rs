//! sql257: `DO $$ BEGIN SELECT now(); END $$;` -- inside a DO block
//! a bare `SELECT` discards its result (DO doesn't return rows). The
//! author probably meant PERFORM (to evaluate side effects) or RAISE
//! NOTICE (to print) or SELECT ... INTO <var>.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql257"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("DO $$") && !upper.contains("DO LANGUAGE") { return }
    let Some(body_start) = body.find("$$").map(|p| p + 2) else { return };
    let body_end = body[body_start..].find("$$").map(|p| body_start + p).unwrap_or(body.len());
    let do_body = &body[body_start..body_end];
    let do_upper = do_body.to_ascii_uppercase();
    let bytes = do_upper.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
      // Find a SELECT token at statement boundary.
      let Some(rel) = do_upper[i..].find("SELECT") else { break };
      let at = i + rel;
      if at > 0 {
        let prev = bytes[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' { i = at + 6; continue }
      }
      // Walk back to the preceding statement boundary (`;`, `BEGIN`, `THEN`, `ELSE`, `LOOP`, start).
      let head = &do_upper[..at].trim_end();
      let prev_kw_end = head.rfind(|c: char| c == ';').map(|p| p + 1).unwrap_or(0);
      let mut between = do_upper[prev_kw_end..at].trim().to_string();
      if between.ends_with("BEGIN") || between.ends_with("THEN") || between.ends_with("ELSE")
        || between.ends_with("LOOP") { between.clear(); }
      if !between.is_empty() && !between.eq_ignore_ascii_case("BEGIN") && !between.eq_ignore_ascii_case("THEN")
        && !between.eq_ignore_ascii_case("ELSE") && !between.eq_ignore_ascii_case("LOOP") {
        i = at + 6;
        continue;
      }
      // SELECT ... INTO ? skip.
      let stmt_text = &do_upper[at..];
      let stmt_end = stmt_text.find(';').unwrap_or(stmt_text.len());
      let stmt_slice = &stmt_text[..stmt_end];
      if stmt_slice.contains("INTO ") { i = at + stmt_end + 1; continue }
      let abs_s = start + body_start + at;
      let abs_e = abs_s + 6;
      out.push(Diagnostic {
        code: "sql257",
        severity: Severity::Warning,
        message: "Bare `SELECT` in DO block discards result -- use `PERFORM expr` (side effects only) or `RAISE NOTICE '%' , expr` to print".into(),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      i = at + stmt_end + 1;
    }
  }
}
