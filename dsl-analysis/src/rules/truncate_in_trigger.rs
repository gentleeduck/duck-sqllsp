//! sql180: `TRUNCATE` inside a trigger function body. PG rejects
//! with `cannot TRUNCATE inside a function`. Heuristic: the
//! statement's source span lives inside a $$ ... $$ block of a
//! CREATE FUNCTION ... RETURNS TRIGGER.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql180"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let Some(rel) = upper.find("TRUNCATE") else { return };
    let kw_at = start + rel;
    // Word-boundary check.
    if let Some(prev) = source.as_bytes().get(kw_at.saturating_sub(1)).copied() {
      if (prev as char).is_ascii_alphanumeric() || prev == b'_' { return; }
    }
    let after = kw_at + "TRUNCATE".len();
    if let Some(next) = source.as_bytes().get(after).copied() {
      if (next as char).is_ascii_alphanumeric() || next == b'_' { return; }
    }
    // Are we inside a CREATE FUNCTION ... RETURNS TRIGGER body?
    if !inside_trigger_function(source, kw_at) {
      return;
    }
    out.push(Diagnostic {
      code: "sql180",
      severity: Severity::Error,
      message: "TRUNCATE inside a trigger function -- PG raises `cannot TRUNCATE inside a function`".into(),
      range: text_size::TextRange::new((kw_at as u32).into(), (after as u32).into()),
    });
  }
}

/// True when `pos` sits inside the body of a CREATE FUNCTION whose
/// RETURNS clause names TRIGGER. Walks back from pos for `$$` body
/// then forward for `RETURNS TRIGGER` between the function header
/// and the `$$`.
fn inside_trigger_function(source: &str, pos: usize) -> bool {
  let upper = source.to_ascii_uppercase();
  let prior = &upper[..pos];
  // Find the most recent $$ opener.
  let Some(dollar_at) = prior.rfind("$$") else { return false };
  // Make sure there's no closing $$ between dollar_at and pos.
  let between = &source[dollar_at + 2..pos];
  if between.contains("$$") { return false; }
  // Find the function header.
  let head = &upper[..dollar_at];
  for kw in ["CREATE OR REPLACE FUNCTION ", "CREATE FUNCTION "] {
    if let Some(at) = head.rfind(kw) {
      let between_head = &head[at..];
      if between_head.contains("RETURNS TRIGGER") {
        return true;
      }
    }
  }
  false
}
