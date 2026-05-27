//! sql297: `NOTIFY chan, '<huge literal>'` -- PG caps NOTIFY
//! payload at NAMEDATALEN-bound length (default 8000 bytes); larger
//! payloads raise 22023 at runtime. Catches the obvious case where
//! the literal in the SQL exceeds the limit.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const LIMIT: usize = 8000;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql297"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("NOTIFY") {
      return;
    }
    let Some(comma_at) = body.find(',') else { return };
    let rest = body[comma_at + 1..].trim_start();
    if !rest.starts_with('\'') {
      return;
    }
    let Some(close_rel) = rest[1..].find('\'') else { return };
    let lit = &rest[1..1 + close_rel];
    if lit.len() <= LIMIT {
      return;
    }
    let abs_s = start + comma_at + 1 + (body[comma_at + 1..].len() - rest.len());
    let abs_e = abs_s + close_rel + 2;
    out.push(Diagnostic {
      code: "sql297",
      severity: Severity::Warning,
      message: format!(
        "NOTIFY payload is {} bytes -- exceeds default 8000-byte limit; PG raises 22023 at runtime",
        lit.len(),
      ),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}
