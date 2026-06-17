//! sql591: `VALUES (1, 2), (3, 4, 5)` -- the rows of a multi-row VALUES list
//! have different lengths. Postgres rejects this with 21000 ("VALUES lists
//! must all be the same length"). Usually a missing or extra column in one
//! row. (sql038 checks each tuple against the INSERT column list; this checks
//! the tuples against each other.)

use crate::clause_scan::{find_clause, find_clause_end, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql591"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let ub = upper.as_bytes();
    let Some(vat) = find_clause(ub, b"VALUES") else { return };
    let vs = vat + 6;
    let ve = find_clause_end(ub, vs, &["ON", "RETURNING", "ORDER", "LIMIT", "OFFSET"]);

    let mut expected: Option<usize> = None;
    for (slice, off) in split_top_level(&body[vs..ve]) {
      let tup = slice.trim();
      if !tup.starts_with('(') || !tup.ends_with(')') {
        continue; // not a clean tuple (e.g. a trailing SELECT) -- bail on this one
      }
      let len = count_top_level(&tup[1..tup.len() - 1]);
      match expected {
        None => expected = Some(len),
        Some(e) if e != len => {
          let lead = slice.len() - slice.trim_start().len();
          out.push(Diagnostic {
            code: "sql591",
            severity: Severity::Error,
            message: format!("this VALUES row has {len} columns but an earlier row has {e} -- all rows must match (PG error 21000)"),
            range: crate::range_at(start + vs + off + lead, start + vs + off + slice.trim_end().len()),
          });
          return;
        },
        _ => {},
      }
    }
  }
}

fn count_top_level(inner: &str) -> usize {
  if inner.trim().is_empty() {
    return 0;
  }
  let bytes = inner.as_bytes();
  let mut count = 1usize;
  let mut depth = 0i32;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
      b'\'' => {
        i += 1;
        while i < bytes.len() && bytes[i] != b'\'' {
          i += 1;
        }
      },
      b',' if depth == 0 => count += 1,
      _ => {},
    }
    i += 1;
  }
  count
}
