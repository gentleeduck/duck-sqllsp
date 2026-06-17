//! sql611: `UPDATE ... ORDER BY` / `DELETE ... ORDER BY` -- PostgreSQL's UPDATE
//! and DELETE don't accept a top-level ORDER BY (it raises 42601 at parse).
//! MySQL allows `ORDER BY ... LIMIT` on UPDATE/DELETE; this is a common port
//! mistake. To affect a bounded, ordered subset, target rows via a subquery:
//! `DELETE FROM t WHERE ctid IN (SELECT ctid FROM t ORDER BY ... LIMIT n)`.
//!
//! Only a depth-0 ORDER BY is flagged, so an ORDER BY inside a subquery (e.g.
//! `SET x = (SELECT ... ORDER BY ... LIMIT 1)`) is left alone.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql611"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, raw) = crate::stmt_body(stmt, source);
    let trim = raw.trim_start().to_ascii_uppercase();
    if !(trim.starts_with("UPDATE") || trim.starts_with("DELETE")) {
      return;
    }
    let cleaned = crate::textutil::strip_noise_full(raw);
    let ub = cleaned.to_ascii_uppercase();
    let b = ub.as_bytes();
    let n = b.len();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < n {
      match b[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        b'O' if depth == 0 && i + 8 <= n && &b[i..i + 8] == b"ORDER BY" => {
          let prev_ok = i == 0 || !(b[i - 1] as char).is_alphanumeric() && b[i - 1] != b'_';
          if prev_ok {
            out.push(Diagnostic {
              code: "sql611",
              severity: Severity::Error,
              message: "UPDATE/DELETE does not accept ORDER BY in PostgreSQL -- PG raises 42601; order rows inside a subquery instead".into(),
              range: crate::range_at(start + i, start + i + 8),
            });
            return;
          }
        }
        _ => {}
      }
      i += 1;
    }
  }
}
