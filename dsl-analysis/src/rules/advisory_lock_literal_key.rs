//! sql247: `pg_advisory_lock(1)` (or `pg_advisory_xact_lock(1)`)
//! with a hard-coded literal key. PG advisory locks are global per
//! key, so two unrelated code paths each calling with `1` will
//! serialize on each other -- a hidden mutex. Hint: derive the
//! key from the resource you actually need to serialize on.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

const FNS: &[&str] = &[
  "pg_advisory_lock", "pg_advisory_xact_lock",
  "pg_advisory_unlock", "pg_try_advisory_lock", "pg_try_advisory_xact_lock",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql247"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let lower = body.to_ascii_lowercase();
    for &fn_name in FNS {
      let needle = format!("{fn_name}(");
      let mut from = 0usize;
      while let Some(rel) = lower[from..].find(&needle) {
        let at = from + rel;
        if at > 0 {
          let prev = body.as_bytes()[at - 1] as char;
          if prev.is_ascii_alphanumeric() || prev == '_' { from = at + needle.len(); continue }
        }
        let open = at + needle.len();
        let Some(close) = find_matching_paren(body, open - 1) else { from = open; continue };
        let args = body[open..close].trim();
        // Numeric literal key?
        if args.parse::<i64>().is_ok() {
          out.push(Diagnostic {
            code: "sql247",
            severity: Severity::Hint,
            message: format!(
              "`{fn_name}({args})` -- literal advisory-lock key is a global mutex; derive key from resource ID (hashtext, tableoid, etc)"
            ),
            range: text_size::TextRange::new(((start + at) as u32).into(), ((start + close + 1) as u32).into()),
          });
        }
        from = close + 1;
      }
    }
  }
}

fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
  let bytes = s.as_bytes();
  let mut depth = 0i32;
  let mut i = open;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => { depth -= 1; if depth == 0 { return Some(i); } }
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' { i += 1 }
      }
      _ => {}
    }
    i += 1;
  }
  None
}
