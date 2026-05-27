//! sql200: `JOIN LATERAL (SELECT ... FROM x) y ON ...` where the
//! inner subquery does NOT reference any outer alias. LATERAL is
//! a no-op there and can be safely removed for clarity.
//!
//! Detect by:
//!   * Find each `LATERAL` keyword + parenthesised body.
//!   * Collect FROM/JOIN aliases that appear before the LATERAL.
//!   * If none of those alias tokens appear inside the LATERAL body,
//!     emit a warning to drop LATERAL.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql200"
  }
  fn default_severity(&self) -> Severity {
    Severity::Info
  }

  fn check(&self, source: &str, stmt: &Statement, scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("LATERAL") {
      let kw_at = from + rel;
      let after = kw_at + "LATERAL".len();
      // boundary check
      if kw_at > 0 {
        let prev = body.as_bytes()[kw_at - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' {
          from = after;
          continue;
        }
      }
      let rest = body[after..].trim_start();
      if !rest.starts_with('(') {
        from = after;
        continue;
      }
      let body_open = after + (body[after..].len() - rest.len());
      let Some(close) = find_matching_paren(body, body_open) else { break };
      let inner = &body[body_open + 1..close];
      let inner_lc = inner.to_ascii_lowercase();
      // Collect outer aliases from scope (sources visible to the join).
      let mut ref_found = false;
      for b in scope.bindings.values() {
        let needle = format!("{}.", b.alias.to_ascii_lowercase());
        if inner_lc.contains(&needle) {
          ref_found = true;
          break;
        }
      }
      if !ref_found {
        out.push(Diagnostic {
          code: "sql200",
          severity: Severity::Info,
          message: "LATERAL has no reference to outer FROM aliases -- LATERAL keyword is unnecessary".into(),
          range: text_size::TextRange::new(((start + kw_at) as u32).into(), ((start + close + 1) as u32).into()),
        });
      }
      from = close + 1;
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
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
