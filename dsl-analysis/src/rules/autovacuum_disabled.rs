//! sql579: `... WITH (autovacuum_enabled = false)` / `ALTER TABLE t SET
//! (autovacuum_enabled = off)` -- turning off autovacuum for a table. Unless a
//! scheduled manual VACUUM/ANALYZE replaces it, the table accumulates dead
//! tuples and stale statistics indefinitely, degrading both bloat and plans.
//! Almost always a temporary tweak that got left in.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql579"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let needle = "autovacuum_enabled";
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find(needle) {
      let at = from + rel;
      from = at + needle.len();
      let mut p = skip_ws(bytes, at + needle.len());
      if bytes.get(p) != Some(&b'=') {
        continue;
      }
      p = skip_ws(bytes, p + 1);
      // value: false / off / 0
      let end = (p + 5).min(bytes.len());
      let val = &lower[p..end];
      let off = val.starts_with("false") || val.starts_with("off") || (bytes.get(p) == Some(&b'0') && !bytes.get(p + 1).is_some_and(|b| b.is_ascii_digit()));
      if off {
        out.push(Diagnostic {
          code: "sql579",
          severity: Severity::Warning,
          message: "autovacuum is disabled for this table -- it will bloat and its stats go stale without a manual VACUUM/ANALYZE schedule".into(),
          range: crate::range_at(start + at, start + (p + 1).min(body.len())),
        });
      }
    }
  }
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
