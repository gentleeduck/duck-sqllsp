//! sql155: `TRUNCATE t RETURNING ...` -- TRUNCATE does not support
//! RETURNING. PG rejects at parse time.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql155"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trimmed = upper.trim_start();
    if !trimmed.starts_with("TRUNCATE") {
      return;
    }
    let Some(rel) = upper.find("RETURNING") else { return };
    let prev_ok = rel == 0 || !is_word(upper.as_bytes()[rel - 1] as char);
    let next_ok = rel + 9 == upper.len() || !is_word(upper.as_bytes()[rel + 9] as char);
    if !(prev_ok && next_ok) {
      return;
    }
    let abs_start = start + rel;
    let abs_end = start + rel + 9;
    out.push(Diagnostic {
      code: "sql155",
      severity: Severity::Error,
      message: "TRUNCATE does not support RETURNING -- PG rejects this at parse time".into(),
      range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
    });
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}
