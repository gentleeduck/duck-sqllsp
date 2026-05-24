//! sql010: UNION / INTERSECT / EXCEPT column-count mismatch.
//!
//! Each arm of a set operation must project the same number of columns.
//! The internal AST does not model UNION yet so we tokenise the statement
//! text, split on the top-level UNION / INTERSECT / EXCEPT keywords, and
//! count comma-separated projection expressions in each arm. Sub-queries
//! inside an arm have their parens skipped so an arm with a column list
//! `SELECT a, (SELECT max(b) FROM t), c` is counted as 3.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;
use text_size::{TextRange, TextSize};

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql010"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: u32 = stmt.range.start().into();
    let end: u32 = stmt.range.end().into();
    let end = (end as usize).min(source.len());
    let slice = &source[start as usize..end];

    let arms = split_set_arms(slice);
    if arms.len() < 2 {
      return;
    }
    let mut counts: Vec<(usize, TextRange)> = arms
      .iter()
      .map(|(s, e)| {
        (
          project_count(&slice[*s..*e]),
          TextRange::new(TextSize::from(start + *s as u32), TextSize::from(start + *e as u32)),
        )
      })
      .collect();
    let first = counts[0].0;
    if first == 0 {
      return;
    } // bail when we cannot determine projection count
    for (n, r) in counts.drain(1..) {
      if n != 0 && n != first {
        out.push(Diagnostic {
          code: "sql010",
          severity: Severity::Warning,
          message: format!("UNION arm projects {n} columns, first arm projected {first}; counts must match"),
          range: r,
        });
      }
    }
  }
}

/// Return byte ranges within `text` for each arm of a top-level set
/// operation. Returns one entry when no UNION/INTERSECT/EXCEPT exists.
fn split_set_arms(text: &str) -> Vec<(usize, usize)> {
  let mut arms = Vec::new();
  let mut last = 0usize;
  let mut i = 0usize;
  let bytes = text.as_bytes();
  let n = bytes.len();

  while i < n {
    let c = bytes[i] as char;
    if c == '\'' {
      i = skip_string(bytes, i);
      continue;
    }
    if c == '-' && i + 1 < n && bytes[i + 1] == b'-' {
      i = skip_line_comment(bytes, i);
      continue;
    }
    if c == '/' && i + 1 < n && bytes[i + 1] == b'*' {
      i = skip_block_comment(bytes, i);
      continue;
    }
    if c == '(' {
      i = skip_parens(bytes, i);
      continue;
    }

    if word_at(bytes, i, b"UNION") || word_at(bytes, i, b"INTERSECT") || word_at(bytes, i, b"EXCEPT") {
      arms.push((last, i));
      // Step past UNION [ALL] / EXCEPT [ALL] / INTERSECT [ALL]
      i += ident_len(bytes, i);
      i = skip_spaces(bytes, i);
      if word_at(bytes, i, b"ALL") || word_at(bytes, i, b"DISTINCT") {
        i += ident_len(bytes, i);
        i = skip_spaces(bytes, i);
      }
      last = i;
      continue;
    }
    i += 1;
  }
  arms.push((last, n));
  arms
}

/// Count top-level comma-separated expressions between SELECT and FROM
/// (or end of slice when no FROM). Returns 0 when the arm has no SELECT.
fn project_count(arm: &str) -> usize {
  let bytes = arm.as_bytes();
  let n = bytes.len();
  let mut i = skip_spaces(bytes, 0);
  // Strip leading `(`
  while i < n && bytes[i] == b'(' {
    i += 1;
    i = skip_spaces(bytes, i);
  }
  if !word_at(bytes, i, b"SELECT") {
    return 0;
  }
  i += ident_len(bytes, i);
  // optional ALL / DISTINCT
  i = skip_spaces(bytes, i);
  if word_at(bytes, i, b"ALL") || word_at(bytes, i, b"DISTINCT") {
    i += ident_len(bytes, i);
  }

  let mut depth = 0i32;
  let mut commas = 0usize;
  let mut saw_token = false;
  while i < n {
    let c = bytes[i] as char;
    if c == '\'' {
      i = skip_string(bytes, i);
      saw_token = true;
      continue;
    }
    if c == '-' && i + 1 < n && bytes[i + 1] == b'-' {
      i = skip_line_comment(bytes, i);
      continue;
    }
    if c == '/' && i + 1 < n && bytes[i + 1] == b'*' {
      i = skip_block_comment(bytes, i);
      continue;
    }
    if c == '(' {
      depth += 1;
      i += 1;
      continue;
    }
    if c == ')' {
      depth -= 1;
      i += 1;
      if depth < 0 {
        break;
      }
      continue;
    }
    if depth == 0 {
      if c == ',' {
        commas += 1;
        i += 1;
        continue;
      }
      // Stop at FROM / WHERE / GROUP / ORDER / LIMIT / UNION / INTERSECT / EXCEPT
      for kw in [
        b"FROM" as &[u8],
        b"WHERE",
        b"GROUP",
        b"ORDER",
        b"LIMIT",
        b"UNION",
        b"INTERSECT",
        b"EXCEPT",
        b"HAVING",
        b"WINDOW",
      ] {
        if word_at(bytes, i, kw) {
          return if saw_token { commas + 1 } else { 0 };
        }
      }
    }
    if !c.is_whitespace() {
      saw_token = true;
    }
    i += 1;
  }
  if saw_token { commas + 1 } else { 0 }
}

fn word_at(bytes: &[u8], i: usize, kw: &[u8]) -> bool {
  if i + kw.len() > bytes.len() {
    return false;
  }
  let head = &bytes[i..i + kw.len()];
  if !head.eq_ignore_ascii_case(kw) {
    return false;
  }
  let after = i + kw.len();
  if after < bytes.len() {
    let c = bytes[after];
    if c.is_ascii_alphanumeric() || c == b'_' {
      return false;
    }
  }
  if i > 0 {
    let p = bytes[i - 1];
    if p.is_ascii_alphanumeric() || p == b'_' {
      return false;
    }
  }
  true
}

fn ident_len(bytes: &[u8], i: usize) -> usize {
  let mut j = i;
  while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
    j += 1;
  }
  j - i
}

fn skip_spaces(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && (bytes[i] as char).is_whitespace() {
    i += 1;
  }
  i
}
fn skip_string(bytes: &[u8], mut i: usize) -> usize {
  i += 1;
  while i < bytes.len() {
    if bytes[i] == b'\'' {
      if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
        i += 2;
        continue;
      }
      return i + 1;
    }
    i += 1;
  }
  i
}
fn skip_line_comment(bytes: &[u8], mut i: usize) -> usize {
  while i < bytes.len() && bytes[i] != b'\n' {
    i += 1;
  }
  i
}
fn skip_block_comment(bytes: &[u8], mut i: usize) -> usize {
  i += 2;
  while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
    i += 1;
  }
  (i + 2).min(bytes.len())
}
fn skip_parens(bytes: &[u8], mut i: usize) -> usize {
  let mut depth = 0i32;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => {
        depth += 1;
        i += 1;
      },
      b')' => {
        depth -= 1;
        i += 1;
        if depth == 0 {
          return i;
        }
      },
      b'\'' => i = skip_string(bytes, i),
      _ => i += 1,
    }
  }
  i
}
