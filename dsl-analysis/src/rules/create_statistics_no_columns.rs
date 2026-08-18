//! sql791: `CREATE STATISTICS name (ndistinct)` (or `dependencies`)
//! with fewer than 2 columns/expressions in the `ON` list -- these
//! statistics kinds require at least 2 to be meaningful. PostgreSQL
//! rejects a 1-column ON list for these kinds.

use crate::clause_scan::{find_clause, split_top_level};
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql791"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE STATISTICS") {
      return;
    }
    let ub = upper.as_bytes();
    let Some(paren) = ub.iter().position(|&b| b == b'(') else { return };
    let Some(kind_close) = match_paren(ub, paren) else { return };
    let kinds_up = &upper[paren + 1..kind_close];
    let needs_two = kinds_up.contains("NDISTINCT") || kinds_up.contains("DEPENDENCIES");
    if !needs_two {
      return;
    }
    let Some(on_rel) = find_clause(&ub[kind_close + 1..], b"ON") else { return };
    let on_at = kind_close + 1 + on_rel;
    let list_start = skip_ws(ub, on_at + 2);
    let list_end = find_clause(&ub[list_start..], b"FROM").map(|r| list_start + r).unwrap_or(ub.len());
    let cols = split_top_level(&body[list_start..list_end]);
    if cols.len() < 2 {
      out.push(Diagnostic {
        code: "sql791",
        severity: Severity::Error,
        message: "ndistinct/dependencies statistics require at least 2 columns or expressions".into(),
        range: crate::range_at(start + list_start, start + list_end),
      });
    }
  }
}

fn match_paren(ub: &[u8], open: usize) -> Option<usize> {
  let mut depth = 0i32;
  let mut i = open;
  while i < ub.len() {
    match ub[i] {
      b'(' => depth += 1,
      b')' => {
        depth -= 1;
        if depth == 0 {
          return Some(i);
        }
      },
      b'\'' => {
        i += 1;
        while i < ub.len() && ub[i] != b'\'' {
          i += 1;
        }
      },
      _ => {},
    }
    i += 1;
  }
  None
}

fn skip_ws(ub: &[u8], mut i: usize) -> usize {
  while i < ub.len() && ub[i].is_ascii_whitespace() {
    i += 1;
  }
  i
}
