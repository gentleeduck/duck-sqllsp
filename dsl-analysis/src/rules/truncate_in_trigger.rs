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
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(rel) = upper.find("TRUNCATE") else { return };
    let kw_at = start + rel;
    // Word-boundary check.
    if let Some(prev) = source.as_bytes().get(kw_at.saturating_sub(1)).copied()
      && ((prev as char).is_ascii_alphanumeric() || prev == b'_')
    {
      return;
    }
    let after = kw_at + "TRUNCATE".len();
    if let Some(next) = source.as_bytes().get(after).copied()
      && ((next as char).is_ascii_alphanumeric() || next == b'_')
    {
      return;
    }
    // Are we inside a CREATE FUNCTION ... RETURNS TRIGGER body?
    if !inside_trigger_function(source, kw_at) {
      return;
    }
    out.push(Diagnostic {
      code: "sql180",
      severity: Severity::Error,
      message: "TRUNCATE inside a trigger function -- PG raises `cannot TRUNCATE inside a function`".into(),
      range: crate::range_at(kw_at, after),
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
  // We're inside an open $$...$$ block iff an odd number of $$
  // delimiters appear before `pos`. Just rfind-ing the most recent
  // $$ was wrong when the most recent one was a *closing* marker --
  // a `CREATE TRIGGER ... OR TRUNCATE ON tbl` statement *after* a
  // CREATE FUNCTION ... $$ body $$ would still see the closing $$
  // as the "opener" and falsely fire.
  let dollar_count = prior.matches("$$").count();
  if dollar_count.is_multiple_of(2) {
    return false;
  }
  // The opener is at the *last* $$ in `prior` -- because the count
  // is odd, this is the unmatched opener.
  let Some(dollar_at) = prior.rfind("$$") else { return false };
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
