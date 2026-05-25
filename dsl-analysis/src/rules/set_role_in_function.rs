//! sql259: `SET ROLE <foo>` inside a CREATE FUNCTION body. Almost
//! never intentional -- SET ROLE persists past the function call
//! (it's session-scoped, not function-scoped) so the caller's role
//! is silently mutated. Use SECURITY DEFINER to run as the function
//! owner, or wrap in `SET LOCAL ROLE` within a BEGIN/COMMIT.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql259"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE") { return }
    if !upper.contains("FUNCTION") && !upper.contains("PROCEDURE") { return }
    let Some(body_start) = body.find("$$").map(|p| p + 2) else { return };
    let body_end = body[body_start..].find("$$").map(|p| body_start + p).unwrap_or(body.len());
    let fbody = &body[body_start..body_end];
    let fupper = fbody.to_ascii_uppercase();
    let bytes = fupper.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = fupper[from..].find("SET ROLE") {
      let at = from + rel;
      if at > 0 {
        let prev = bytes[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' { from = at + 8; continue }
      }
      // Skip SET LOCAL ROLE (intentional, tx-scoped).
      let head: String = fupper[..at].chars().rev().take(10).collect::<String>().chars().rev().collect();
      if head.contains("LOCAL ") { from = at + 8; continue }
      let abs_s = start + body_start + at;
      let abs_e = abs_s + "SET ROLE".len();
      out.push(Diagnostic {
        code: "sql259",
        severity: Severity::Warning,
        message: "SET ROLE in function body persists past the call (session-scoped) -- use SET LOCAL ROLE or SECURITY DEFINER".into(),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      from = at + 8;
    }
  }
}
