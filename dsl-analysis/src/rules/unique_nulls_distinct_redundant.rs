//! sql790: `col type NOT NULL UNIQUE NULLS NOT DISTINCT` -- the column
//! is already NOT NULL, so NULLS NOT DISTINCT (which only changes how
//! multiple NULLs are treated by the unique constraint) can never
//! apply; it's a no-op clause. Only the column-level inline form is
//! checked -- a table-level `UNIQUE NULLS NOT DISTINCT (col)`
//! constraint would need cross-referencing the column's own NOT NULL,
//! out of scope for this first pass.

use crate::clause_scan::split_top_level;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql790"
  }
  fn default_severity(&self) -> Severity {
    Severity::Hint
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    if !upper.trim_start().starts_with("CREATE TABLE") {
      return;
    }
    let ub = upper.as_bytes();
    let Some(open) = ub.iter().position(|&b| b == b'(') else { return };
    let Some(close) = match_paren(ub, open) else { return };
    let list_up = &upper[open + 1..close];
    for (entry, off) in split_top_level(list_up) {
      if entry.contains("NOT NULL") && entry.contains("NULLS NOT DISTINCT") {
        let lead = entry.len() - entry.trim_start().len();
        let abs = open + 1 + off + lead;
        let trimmed_len = entry.trim().len();
        out.push(Diagnostic {
          code: "sql790",
          severity: Severity::Hint,
          message: "NULLS NOT DISTINCT is a no-op here -- the column is already NOT NULL".into(),
          range: crate::range_at(start + abs, start + abs + trimmed_len),
        });
      }
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
