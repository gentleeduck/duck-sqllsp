//! sql558: a `CREATE TABLE` with more than one PRIMARY KEY definition (e.g. an
//! inline `id int PRIMARY KEY` plus a table-level `PRIMARY KEY (...)`, or two
//! inline ones). A table may have only one primary key; Postgres rejects it
//! with 42P16 ("multiple primary keys for table ... are not allowed"). For a
//! composite key, list the columns in a single `PRIMARY KEY (a, b)`.

use crate::clause_scan::is_word;
use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql558"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let (start, _body, upper) = crate::stmt_body_upper(stmt, source);
    let Some((bs, be)) = create_table_body(&upper) else { return };
    let ub = upper.as_bytes();

    // Positions of each `PRIMARY KEY` phrase at the column-list's top level.
    let mut pks = Vec::new();
    let mut depth = 0i32;
    let mut i = bs;
    while i < be {
      match ub[i] {
        b'(' | b'[' => depth += 1,
        b')' | b']' => depth -= 1,
        b'\'' => {
          i += 1;
          while i < be && ub[i] != b'\'' {
            i += 1;
          }
        },
        b'P' if depth == 0 && primary_key_at(ub, i, be) => {
          pks.push(i);
          i += 7; // past PRIMARY
          continue;
        },
        _ => {},
      }
      i += 1;
    }

    if pks.len() >= 2 {
      let at = pks[1];
      out.push(Diagnostic {
        code: "sql558",
        severity: Severity::Error,
        message: "multiple PRIMARY KEY definitions -- a table may have only one (PG error 42P16)".into(),
        range: crate::range_at(start + at, start + (at + 11).min(be)),
      });
    }
  }
}

/// Word-bounded `PRIMARY` followed (after whitespace) by `KEY` at `i`.
fn primary_key_at(ub: &[u8], i: usize, to: usize) -> bool {
  if i + 7 > to || &ub[i..i + 7] != b"PRIMARY" || (i > 0 && is_word(ub[i - 1] as char)) {
    return false;
  }
  let mut j = i + 7;
  if j < to && !ub[j].is_ascii_whitespace() {
    return false;
  }
  while j < to && ub[j].is_ascii_whitespace() {
    j += 1;
  }
  j + 3 <= to && &ub[j..j + 3] == b"KEY" && (j + 3 == to || !is_word(ub[j + 3] as char))
}

fn create_table_body(upper: &str) -> Option<(usize, usize)> {
  let at = upper.find("CREATE TABLE")?;
  let bytes = upper.as_bytes();
  let open = bytes[at..].iter().position(|&b| b == b'(')? + at;
  if upper[at..open].contains(" AS ") {
    return None;
  }
  let close = match_paren(bytes, open)?;
  Some((open + 1, close))
}

fn match_paren(bytes: &[u8], open: usize) -> Option<usize> {
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
