//! sql284: `TG_OP`, `TG_TABLE_NAME`, `TG_RELID`, `TG_NAME`, `TG_WHEN`,
//! `TG_LEVEL`, `TG_NARGS`, `TG_ARGV` referenced inside a CREATE
//! FUNCTION body that doesn't return TRIGGER. PG raises 42703 at
//! runtime -- the TG_* vars are only bound in trigger functions.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const TG_VARS: &[&str] = &[
  "TG_OP", "TG_TABLE_NAME", "TG_TABLE_SCHEMA", "TG_RELID",
  "TG_NAME", "TG_WHEN", "TG_LEVEL", "TG_NARGS", "TG_ARGV",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql284"
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
    if upper.contains("RETURNS TRIGGER") || upper.contains("RETURNS EVENT_TRIGGER") { return }
    let Some(body_start) = body.find("$$").map(|p| p + 2) else { return };
    let body_end = body[body_start..].find("$$").map(|p| body_start + p).unwrap_or(body.len());
    let fbody = &body[body_start..body_end];
    let fupper = fbody.to_ascii_uppercase();
    for var in TG_VARS {
      let mut from = 0usize;
      while let Some(rel) = fupper[from..].find(var) {
        let at = from + rel;
        if at > 0 {
          let prev = fupper.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' { from = at + var.len(); continue }
        }
        let after = at + var.len();
        if after < fupper.len() {
          let next = fupper.as_bytes()[after] as char;
          if next.is_ascii_alphanumeric() || next == '_' { from = after; continue }
        }
        let abs_s = start + body_start + at;
        let abs_e = abs_s + var.len();
        out.push(Diagnostic {
          code: "sql284",
          severity: Severity::Error,
          message: format!(
            "`{var}` referenced in non-TRIGGER function -- TG_* vars only exist when RETURNS TRIGGER; PG raises 42703"
          ),
          range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
        });
        from = after;
      }
    }
  }
}
