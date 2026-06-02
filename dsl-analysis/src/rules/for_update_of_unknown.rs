//! sql192: `SELECT ... FROM a JOIN b ... FOR UPDATE OF x` where
//! `x` is not in the FROM list (neither table name nor alias).
//! PG raises 42P01 "relation `x` in FOR UPDATE clause not found in
//! FROM clause" at runtime.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql192"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::Select(_) = &stmt.kind else { return };
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    // Iterate over both FOR UPDATE OF and FOR SHARE OF.
    for needle in ["FOR UPDATE OF ", "FOR SHARE OF ", "FOR NO KEY UPDATE OF ", "FOR KEY SHARE OF "] {
      let Some(rel) = upper.find(needle) else { continue };
      let after = rel + needle.len();
      // Read comma-separated identifier list until end / NOWAIT / SKIP LOCKED / ;.
      let tail_upper = &upper[after..];
      let mut stop = tail_upper.len();
      for term in [" NOWAIT", " SKIP LOCKED", ";", "\n"] {
        if let Some(p) = tail_upper.find(term) {
          stop = stop.min(p);
        }
      }
      let list = &body[after..after + stop];
      for raw in list.split(',') {
        let name = raw.trim().trim_matches('"');
        if name.is_empty() {
          continue;
        }
        let bare = name.rsplit('.').next().unwrap_or(name);
        let known = scope
          .bindings
          .values()
          .any(|b| b.alias.eq_ignore_ascii_case(bare) || b.table.name.eq_ignore_ascii_case(bare));
        if known {
          continue;
        }
        // Locate the offending name in the source span.
        let off = list.find(name).unwrap_or(0);
        let abs_s = start + after + off;
        let abs_e = abs_s + name.len();
        out.push(Diagnostic {
          code: "sql192",
          severity: Severity::Error,
          message: format!("`FOR UPDATE OF {name}` -- `{bare}` not in FROM list, PG raises 42P01"),
          range: crate::range_at(abs_s, abs_e),
        });
      }
      return;
    }
  }
}
