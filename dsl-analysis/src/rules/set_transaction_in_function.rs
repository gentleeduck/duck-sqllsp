//! sql275: `SET TRANSACTION ...` (READ ONLY / READ WRITE / ISOLATION LEVEL)
//! inside a CREATE FUNCTION body. Function bodies run inside the
//! caller's open tx and cannot mutate transaction characteristics
//! mid-flight. PG raises 25001 "SET TRANSACTION ISOLATION LEVEL must
//! be called before any query" at runtime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql275"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.contains("CREATE") || !upper.contains("FUNCTION") { return }
    let Some(body_start) = body.find("$$").map(|p| p + 2) else { return };
    let body_end = body[body_start..].find("$$").map(|p| body_start + p).unwrap_or(body.len());
    let fbody = &body[body_start..body_end];
    let fupper = fbody.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = fupper[from..].find("SET TRANSACTION") {
      let at = from + rel;
      if at > 0 {
        let prev = fupper.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' { from = at + 15; continue }
      }
      let abs_s = start + body_start + at;
      let abs_e = abs_s + "SET TRANSACTION".len();
      out.push(Diagnostic {
        code: "sql275",
        severity: Severity::Error,
        message: "SET TRANSACTION inside function body -- functions run in the caller's tx and cannot change isolation/access mode mid-flight; PG raises 25001".into(),
        range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
      });
      from = at + 15;
    }
  }
}
