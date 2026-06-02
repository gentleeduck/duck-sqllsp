//! sql216: `INSERT INTO t VALUES (1,2), (1,2,3)` -- the rows in a
//! VALUES list disagree on column count. PG raises 42601 / 42P10 at
//! parse / execute time. Text-scan: locate the VALUES keyword,
//! split top-level paren-wrapped tuples, count commas at the
//! tuple's depth-1 level.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql216"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(v_at) = upper.find("VALUES") else { return };
    // boundary
    if v_at > 0 {
      let prev = body.as_bytes()[v_at - 1] as char;
      if prev.is_ascii_alphanumeric() || prev == '_' {
        return;
      }
    }
    let after = v_at + "VALUES".len();
    // Collect each (...) tuple after VALUES.
    let bytes = body.as_bytes();
    let mut i = after;
    let mut widths: Vec<(usize, usize, usize)> = Vec::new(); // (open_abs, close_abs, width)
    while i < bytes.len() {
      while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1
      }
      if i >= bytes.len() || bytes[i] != b'(' {
        break;
      }
      let open = i;
      let close = find_matching_paren(body, open);
      let Some(close) = close else { break };
      let inner = &body[open + 1..close];
      let width = 1 + count_top_level_commas(inner);
      widths.push((open, close, width));
      i = close + 1;
      while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1
      }
      if i < bytes.len() && bytes[i] == b',' {
        i += 1;
        continue;
      }
      break;
    }
    if widths.len() < 2 {
      return;
    }
    let first_w = widths[0].2;
    for (open, close, w) in widths.iter().skip(1) {
      if *w != first_w {
        let abs_s = start + open;
        let abs_e = start + close + 1;
        out.push(Diagnostic {
          code: "sql216",
          severity: Severity::Error,
          message: format!("VALUES row has {w} columns; first row has {first_w} -- all VALUES rows must match width"),
          range: crate::range_at(abs_s, abs_e),
        });
      }
    }
  }
}

fn count_top_level_commas(text: &str) -> usize {
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut commas = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      // Treat `[`/`]` like parens so commas inside `ARRAY[ROW(a,b),
      // ROW(c,d)]` don't count as separate VALUES tuple values.
      b'(' | b'[' => depth += 1,
      b')' | b']' => depth -= 1,
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
