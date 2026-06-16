//! sql602: `DECODE(expr, search, result [, ...] [, default])` -- Oracle's
//! if-then-else function. PostgreSQL has no such function; use a `CASE`
//! expression (or `COALESCE`/`NULLIF` for the simple cases).
//!
//! PostgreSQL *does* have a two-argument `decode(text, format)` for binary
//! decoding (base64/hex), so only calls with three or more top-level arguments
//! -- the Oracle signature -- are flagged.

use crate::clause_scan::split_top_level;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql602"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body) = crate::stmt_body(stmt, source);
    let lower = body.to_ascii_lowercase();
    let bytes = body.as_bytes();
    let n = bytes.len();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("decode(") {
      let at = from + rel;
      from = at + 7;
      if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
        continue;
      }
      // walk the balanced parentheses after `decode`
      let open = at + 6;
      let mut depth = 0i32;
      let mut j = open;
      let mut close = None;
      while j < n {
        match bytes[j] {
          b'(' => depth += 1,
          b')' => {
            depth -= 1;
            if depth == 0 {
              close = Some(j);
              break;
            }
          }
          _ => {}
        }
        j += 1;
      }
      let Some(close) = close else { continue };
      let args = &body[open + 1..close];
      if split_top_level(args).len() >= 3 {
        out.push(Diagnostic {
          code: "sql602",
          severity: Severity::Error,
          message: "`DECODE` is an Oracle function -- PostgreSQL uses a CASE expression".into(),
          range: crate::range_at(start + at, start + at + 6),
        });
      }
    }
  }
}
