//! sql307: `UPDATE ... LIMIT N` / `DELETE ... LIMIT N` -- PG does
//! not support LIMIT on UPDATE or DELETE (only SELECT). PG raises
//! 42601 at parse. MySQL allows it; common port mistake. Suggest
//! `UPDATE ... WHERE ctid IN (SELECT ctid FROM t WHERE ... LIMIT N)`.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql307"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let trim = upper.trim_start();
    if !(trim.starts_with("UPDATE") || trim.starts_with("DELETE")) {
      return;
    }
    // Need to find LIMIT at the top level (not inside a subquery).
    let bytes = body.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < bytes.len() {
      match bytes[i] {
        b'(' => depth += 1,
        b')' => depth -= 1,
        b'\'' => {
          i += 1;
          while i < bytes.len() && bytes[i] != b'\'' {
            i += 1
          }
        },
        _ => {
          if depth == 0 && i + 6 <= bytes.len() && upper[i..].starts_with("LIMIT ") {
            let prev_ok = i == 0
              || !{
                let p = bytes[i - 1] as char;
                p.is_ascii_alphanumeric() || p == '_'
              };
            if prev_ok {
              let abs_s = start + i;
              let abs_e = abs_s + "LIMIT".len();
              let kind = if trim.starts_with("UPDATE") { "UPDATE" } else { "DELETE" };
              out.push(Diagnostic {
                code: "sql307",
                severity: Severity::Error,
                message: format!(
                  "PG does not support LIMIT on {kind} -- use `WHERE ctid IN (SELECT ctid FROM t WHERE ... LIMIT N)` (PG 42601)"
                ),
                range: crate::range_at(abs_s, abs_e),
              });
              return;
            }
          }
        },
      }
      i += 1;
    }
  }
}
