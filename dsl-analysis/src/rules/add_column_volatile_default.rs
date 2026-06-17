//! sql587: `ALTER TABLE t ADD COLUMN c uuid DEFAULT gen_random_uuid()` -- a
//! non-constant default on ADD COLUMN forces a full table rewrite under an
//! ACCESS EXCLUSIVE lock (the constant-default fast path only applies to a
//! literal). On a large table that's a long outage. Add the column with no
//! default, backfill in batches, then `SET DEFAULT`. (sql145 covers the
//! per-row-recompute semantics; this is the rewrite/lock cost.)

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

const VOLATILE_FNS: &[&str] = &[
  "now(",
  "random(",
  "gen_random_uuid(",
  "uuid_generate_v4(",
  "clock_timestamp(",
  "statement_timestamp(",
  "transaction_timestamp(",
  "timeofday(",
  "nextval(",
];

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql587"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.contains("ALTER TABLE") || !upper.contains("ADD") {
      return;
    }
    let lower = body.to_ascii_lowercase();
    // For each DEFAULT in an ADD COLUMN, check its expression for a volatile fn.
    let ub = upper.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = upper[from..].find("DEFAULT") {
      let at = from + rel;
      from = at + 7;
      if (at > 0 && is_word(ub[at - 1])) || ub.get(at + 7).is_some_and(|&b| is_word(b)) {
        continue;
      }
      // Default expression runs to the next top-level comma / end.
      let expr_end = top_level_comma_or_end(body, at + 7);
      let expr = &lower[at + 7..expr_end];
      if let Some(f) = VOLATILE_FNS.iter().find(|f| expr.contains(**f)) {
        out.push(Diagnostic {
          code: "sql587",
          severity: Severity::Warning,
          message: format!(
            "ADD COLUMN with a non-constant default (`{}`) rewrites the whole table under an ACCESS EXCLUSIVE lock -- add the column, backfill, then SET DEFAULT",
            f.trim_end_matches('(')
          ),
          range: crate::range_at(start + at, start + expr_end.min(body.len())),
        });
      }
    }
  }
}

fn top_level_comma_or_end(body: &str, from: usize) -> usize {
  let bytes = body.as_bytes();
  let mut depth = 0i32;
  let mut i = from;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' if depth == 0 => return i,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => return i,
      b';' if depth == 0 => return i,
      _ => {},
    }
    i += 1;
  }
  bytes.len()
}

fn is_word(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}
