//! sql240: `SAVEPOINT s; ... SAVEPOINT s;` -- declaring the same
//! savepoint name twice inside one transaction. PG allows it: the
//! second SAVEPOINT shadows the first (so ROLLBACK TO s rolls back
//! only to the inner). Almost always a copy-paste mistake.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql240"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let body_owned = crate::textutil::strip_noise_full(raw);
    let body = body_owned.as_str();
    let upper = body.to_ascii_uppercase();
    if !upper.trim_start().starts_with("SAVEPOINT") {
      return;
    }
    let after = upper.find("SAVEPOINT ").map(|p| p + "SAVEPOINT ".len());
    let Some(after) = after else { return };
    let rest = &body[after..];
    let name_end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '"').unwrap_or(rest.len());
    let name = rest[..name_end].trim_matches('"').to_string();
    if name.is_empty() {
      return;
    }
    // Check the prelude for an unreleased SAVEPOINT <name> in the same tx.
    let prelude_upper = source[..start].to_ascii_uppercase();
    let mut search_from = 0usize;
    let mut last_sp_at: Option<usize> = None;
    let mut last_release_at: Option<usize> = None;
    while let Some(rel) = prelude_upper[search_from..].find(&format!("SAVEPOINT {}", name.to_ascii_uppercase())) {
      let at = search_from + rel;
      if at > 0 {
        let prev = prelude_upper.as_bytes()[at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          search_from = at + 10;
          continue;
        }
      }
      // Reject "RELEASE SAVEPOINT" or "ROLLBACK TO SAVEPOINT".
      let head: String = prelude_upper[..at].chars().rev().take(20).collect::<String>().chars().rev().collect();
      if head.ends_with("RELEASE ") || head.ends_with("TO ") {
        search_from = at + 10;
        continue;
      }
      last_sp_at = Some(at);
      search_from = at + 10;
    }
    let needle_rel = format!("RELEASE SAVEPOINT {}", name.to_ascii_uppercase());
    let needle_rb = format!("ROLLBACK TO SAVEPOINT {}", name.to_ascii_uppercase());
    if let Some(at) = prelude_upper.rfind(&needle_rel) {
      last_release_at = Some(at);
    }
    if let Some(at) = prelude_upper.rfind(&needle_rb) {
      last_release_at = Some(last_release_at.map_or(at, |x| x.max(at)));
    }
    let Some(sp_at) = last_sp_at else { return };
    if let Some(rel_at) = last_release_at
      && rel_at > sp_at
    {
      return;
    }
    let abs_s = start + after;
    let abs_e = abs_s + name_end;
    out.push(Diagnostic {
      code: "sql240",
      severity: Severity::Warning,
      message: format!(
        "SAVEPOINT `{name}` was already declared without RELEASE -- second SAVEPOINT shadows first; ROLLBACK TO `{name}` will only rewind to inner"
      ),
      range: crate::range_at(abs_s, abs_e),
    });
  }
}
