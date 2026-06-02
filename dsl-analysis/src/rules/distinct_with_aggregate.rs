//! sql093: `SELECT DISTINCT count(...) FROM t` -- DISTINCT after an
//! aggregate without GROUP BY is almost always redundant or wrong.
//! Aggregates already collapse rows; DISTINCT on a single-row result
//! does nothing.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql093"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    if !matches!(stmt.kind, StatementKind::Select(_)) {
      return;
    }
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    // Only check the OUTERMOST SELECT projection. The previous text
    // scan over the whole body wrongly combined DISTINCT in one
    // subquery with an aggregate in another.
    let bytes = upper.as_bytes();
    let n = bytes.len();
    let select_pos = upper.trim_start().find("SELECT").map(|p| (upper.len() - upper.trim_start().len()) + p);
    let Some(select_pos) = select_pos else { return };
    let after_sel = select_pos + "SELECT".len();
    // Find depth-0 FROM after the SELECT.
    let mut depth = 0i32;
    let mut from_at: Option<usize> = None;
    let mut i = after_sel;
    while i + 4 <= n {
      match bytes[i] {
        b'(' => {
          depth += 1;
          i += 1;
          continue;
        },
        b')' => {
          depth -= 1;
          i += 1;
          continue;
        },
        b'\'' => {
          i += 1;
          while i < n && bytes[i] != b'\'' {
            i += 1
          }
          if i < n {
            i += 1
          }
          continue;
        },
        _ => {},
      }
      if depth == 0 && i + 5 <= n && &upper[i..i + 5] == " FROM" {
        from_at = Some(i + 1);
        break;
      }
      i += 1;
    }
    let proj_end = from_at.unwrap_or(n);
    let proj = &upper[after_sel..proj_end];
    if !proj.trim_start().starts_with("DISTINCT") {
      return;
    }
    // Has GROUP BY anywhere at depth 0?
    if has_group_by_top(&upper, from_at.unwrap_or(n)) {
      return;
    }
    // Look for any aggregate function call in the top-level
    // projection only.
    const AGGS: &[&str] = &[
      "COUNT(",
      "SUM(",
      "AVG(",
      "MIN(",
      "MAX(",
      "ARRAY_AGG(",
      "STRING_AGG(",
      "JSON_AGG(",
      "JSONB_AGG(",
      "BOOL_AND(",
      "BOOL_OR(",
      "EVERY(",
    ];
    if !AGGS.iter().any(|a| proj.contains(a)) {
      return;
    }
    let Some(distinct_pos) = upper.find("DISTINCT") else { return };
    let abs_start = start + distinct_pos;
    let abs_end = abs_start + 8;
    out.push(Diagnostic {
      code: "sql093",
      severity: Severity::Warning,
      message: "SELECT DISTINCT with an aggregate but no GROUP BY -- DISTINCT is redundant on collapsed rows".into(),
      range: crate::range_at(abs_start, abs_end),
    });
  }
}

fn has_group_by_top(upper: &str, from: usize) -> bool {
  let bytes = upper.as_bytes();
  let n = bytes.len();
  let mut depth = 0i32;
  let mut i = from;
  while i + 8 <= n {
    match bytes[i] {
      b'(' => {
        depth += 1;
        i += 1;
        continue;
      },
      b')' => {
        depth -= 1;
        i += 1;
        continue;
      },
      b'\'' => {
        i += 1;
        while i < n && bytes[i] != b'\'' {
          i += 1
        }
        if i < n {
          i += 1
        }
        continue;
      },
      _ => {},
    }
    if depth == 0 && &upper[i..i + 8] == "GROUP BY" {
      let prev_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
      if prev_ok {
        return true;
      }
    }
    i += 1;
  }
  false
}
