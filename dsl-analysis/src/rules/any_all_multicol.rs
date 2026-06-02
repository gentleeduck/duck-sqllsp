//! sql228: `x = ANY (SELECT 1, 2 FROM ...)` -- the subquery on the
//! RHS of an ANY/ALL/IN must return exactly one column. PG raises
//! 42601 at parse time. Counts top-level commas in the subquery
//! projection.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql228"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    for kw in ["= ANY (", "<> ANY (", "= ALL (", "<> ALL (", " IN ("] {
      let mut from = 0usize;
      while let Some(rel) = upper[from..].find(kw) {
        let at = from + rel;
        let open = at + kw.len() - 1;
        let Some(close) = find_matching_paren(body, open) else { break };
        let inner = &body[open + 1..close];
        let inner_upper = inner.to_ascii_uppercase();
        if !inner_upper.trim_start().starts_with("SELECT") {
          from = close + 1;
          continue;
        }
        let proj_end = inner_upper.find(" FROM ").unwrap_or(inner.len());
        let proj = &inner[7..proj_end].trim();
        let cols = 1 + count_top_level_commas(proj);
        // Row-constructor LHS: `(a, b, c) IN (SELECT ... )` -- the
        // tuple width must match the subquery column count. Count
        // the LHS row width and require equality, not exactly 1.
        let lhs_width = lhs_row_width(body, at);
        let required = lhs_width.unwrap_or(1);
        if cols != required && !proj.contains('*') {
          let msg = if required == 1 {
            format!("ANY/ALL/IN subquery returns {cols} columns -- exactly 1 required (PG 42601)")
          } else {
            format!("ANY/ALL/IN subquery returns {cols} columns -- LHS row constructor has {required} (PG 42601)")
          };
          out.push(Diagnostic {
            code: "sql228",
            severity: Severity::Error,
            message: msg,
            range: crate::range_at(start + open, start + close + 1),
          });
        }
        from = close + 1;
      }
    }
  }
}

/// When the keyword sits right after a `)`, walk back to its matching
/// `(` and count top-level commas inside -- giving the LHS row-width.
/// Returns None when LHS is not a paren-wrapped row.
fn lhs_row_width(body: &str, kw_at: usize) -> Option<usize> {
  let bytes = body.as_bytes();
  let mut j = kw_at;
  while j > 0 && bytes[j - 1].is_ascii_whitespace() {
    j -= 1;
  }
  // Step back through an optional `NOT` keyword so `(a, b) NOT IN (...)`
  // detects the LHS row constructor too.
  if j >= 3 && body[..j].to_ascii_uppercase().ends_with("NOT") {
    let nstart = j - 3;
    let prev_is_word = nstart > 0 && (bytes[nstart - 1].is_ascii_alphanumeric() || bytes[nstart - 1] == b'_');
    if !prev_is_word {
      j = nstart;
      while j > 0 && bytes[j - 1].is_ascii_whitespace() {
        j -= 1;
      }
    }
  }
  if j == 0 || bytes[j - 1] != b')' {
    return None;
  }
  let close = j - 1;
  let mut depth = 1i32;
  let mut i = close;
  while i > 0 {
    i -= 1;
    match bytes[i] {
      b')' => depth += 1,
      b'(' => {
        depth -= 1;
        if depth == 0 {
          let inner = &body[i + 1..close];
          return Some(1 + count_top_level_commas(inner));
        }
      },
      _ => {},
    }
  }
  None
}

fn count_top_level_commas(text: &str) -> usize {
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut commas = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => commas += 1,
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
  commas
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
