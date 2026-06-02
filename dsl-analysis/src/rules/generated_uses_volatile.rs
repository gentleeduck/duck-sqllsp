//! sql193: `GENERATED ALWAYS AS (expr) STORED` where `expr` calls a
//! known-volatile function (random / now / clock_timestamp / uuid /
//! nextval / etc). PG raises 42P17 "generation expression is not
//! immutable" at CREATE TABLE time.
//!
//! Textual: scans CREATE TABLE bodies for `GENERATED ALWAYS AS (...)
//! STORED` and looks for volatile call names in the parenthesised
//! expression.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

const VOLATILE: &[&str] = &[
  "random",
  "now",
  "clock_timestamp",
  "statement_timestamp",
  "transaction_timestamp",
  "current_timestamp",
  "current_time",
  "current_date",
  "localtime",
  "localtimestamp",
  "gen_random_uuid",
  "uuid_generate_v1",
  "uuid_generate_v4",
  "nextval",
  "currval",
  "lastval",
  "setval",
  "txid_current",
  "pg_backend_pid",
  "pg_advisory_lock",
  "session_user",
  "current_user",
  "current_role",
];

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql193"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(&stmt.kind, StatementKind::CreateTable(_)) {
      return;
    }
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("GENERATED ALWAYS AS") {
      let at = from + rel;
      let after = at + "GENERATED ALWAYS AS".len();
      let rest = body[after..].trim_start();
      if !rest.starts_with('(') {
        from = after;
        continue;
      }
      let abs_open = after + (body[after..].len() - rest.len());
      let Some(close) = find_matching_paren(body, abs_open) else {
        from = after;
        continue;
      };
      let expr = &body[abs_open + 1..close];
      let trailing = body[close + 1..].trim_start();
      if !trailing.to_ascii_uppercase().starts_with("STORED") {
        from = close;
        continue;
      }
      let expr_lc = expr.to_ascii_lowercase();
      for v in VOLATILE {
        let needle = format!("{v}(");
        if expr_lc.contains(&needle) {
          out.push(Diagnostic {
            code: "sql193",
            severity: Severity::Error,
            message: format!(
              "GENERATED ALWAYS AS (...) STORED calls volatile `{v}()` -- PG raises 42P17, expression must be IMMUTABLE"
            ),
            range: crate::range_at(start + at, start + close + 1),
          });
          break;
        }
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
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}
