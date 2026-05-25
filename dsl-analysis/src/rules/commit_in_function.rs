//! sql219: `COMMIT` / `ROLLBACK` inside a PL/pgSQL FUNCTION body.
//! PG only allows transaction control statements inside PROCEDUREs;
//! functions get 2D000 "invalid transaction termination" at runtime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql219"
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
    // Only fire for FUNCTION (not PROCEDURE).
    if upper.contains("CREATE PROCEDURE") || upper.contains("CREATE OR REPLACE PROCEDURE") { return }
    if !upper.contains("LANGUAGE PLPGSQL") { return }
    let Some(body_start) = body.find("$$").map(|p| p + 2) else { return };
    let body_end = body[body_start..].find("$$").map(|p| body_start + p).unwrap_or(body.len());
    let fbody = &body[body_start..body_end];
    let fupper = fbody.to_ascii_uppercase();
    for kw in ["COMMIT", "ROLLBACK"] {
      let bytes = fupper.as_bytes();
      let mut from = 0usize;
      while let Some(rel) = fupper[from..].find(kw) {
        let at = from + rel;
        if at > 0 {
          let prev = bytes[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' { from = at + kw.len(); continue }
        }
        let after = at + kw.len();
        if after < bytes.len() {
          let next = bytes[after] as char;
          if next.is_ascii_alphanumeric() || next == '_' { from = after; continue }
        }
        // Skip ROLLBACK TO (savepoint form is allowed).
        if kw == "ROLLBACK" {
          let tail = fupper[after..].trim_start();
          if tail.starts_with("TO") { from = after; continue }
        }
        let abs_s = start + body_start + at;
        let abs_e = abs_s + kw.len();
        out.push(Diagnostic {
          code: "sql219",
          severity: Severity::Error,
          message: format!(
            "`{kw}` inside PL/pgSQL FUNCTION body -- only PROCEDUREs may control transactions; PG raises 2D000"
          ),
          range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
        from = after;
      }
    }
  }
}
