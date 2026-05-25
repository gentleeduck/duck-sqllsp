//! sql294: `BEGIN;` (or `START TRANSACTION;`) when an earlier
//! BEGIN in the source hasn't been COMMITed / ROLLBACKed. PG emits
//! WARNING "there is already a transaction in progress". The
//! author probably meant SAVEPOINT for a nested rollback unit.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql294"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let trim = upper.trim_start();
    let is_begin = trim.starts_with("BEGIN") && !trim.starts_with("BEGIN ATOMIC");
    let is_start = trim.starts_with("START TRANSACTION");
    if !(is_begin || is_start) { return }
    // Skip BEGIN inside plpgsql body bracket (DO $$, CREATE FUNCTION).
    let prelude = &source[..start];
    if inside_dollar(prelude) { return }
    let prelude_upper = prelude.to_ascii_uppercase();
    let begins = count_kw(&prelude_upper, "BEGIN") + count_phrase(&prelude_upper, "START TRANSACTION");
    let closes = count_kw(&prelude_upper, "COMMIT") + count_kw(&prelude_upper, "ROLLBACK") + count_phrase(&prelude_upper, "END TRANSACTION");
    if begins <= closes { return }
    let lead = body.len() - body.trim_start().len();
    let abs_s = start + lead;
    let abs_e = abs_s + if is_begin { "BEGIN".len() } else { "START TRANSACTION".len() };
    out.push(Diagnostic {
      code: "sql294",
      severity: Severity::Warning,
      message: "Nested BEGIN -- a transaction is already open; PG warns and ignores. For a rollback-only unit use SAVEPOINT".into(),
      range: text_size::TextRange::new((abs_s as u32).into(), (abs_e as u32).into()),
    });
  }
}

fn inside_dollar(prefix: &str) -> bool {
  prefix.matches("$$").count() % 2 == 1
}

fn count_kw(s: &str, needle: &str) -> usize {
  let bytes = s.as_bytes();
  let mut from = 0usize;
  let mut n = 0usize;
  while let Some(rel) = s[from..].find(needle) {
    let at = from + rel;
    let before_ok = at == 0 || !{ let p = bytes[at - 1] as char; p.is_ascii_alphanumeric() || p == '_' };
    let after = at + needle.len();
    let after_ok = after >= bytes.len() || !{ let p = bytes[after] as char; p.is_ascii_alphanumeric() || p == '_' };
    if before_ok && after_ok { n += 1 }
    from = at + needle.len();
  }
  n
}

fn count_phrase(s: &str, needle: &str) -> usize { s.matches(needle).count() }
