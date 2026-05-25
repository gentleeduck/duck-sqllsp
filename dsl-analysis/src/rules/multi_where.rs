//! sql098: more than one `WHERE` clause in the same statement (outside
//! parentheses/subqueries). Usually a copy/paste mistake -- PG rejects
//! at parse time.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::Statement;
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql098"
  }
  fn default_severity(&self) -> Severity {
    Severity::Error
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let start: usize = u32::from(stmt.range.start()) as usize;
    let end: usize = (u32::from(stmt.range.end()) as usize).min(source.len());
    let body = &source[start..end];
    let upper = body.to_ascii_uppercase();
    // Skip PL/pgSQL function/procedure bodies + DO blocks -- multiple
    // top-level WHEREs there belong to different sub-statements.
    if upper.contains("$$") || upper.contains("LANGUAGE PLPGSQL") || upper.contains("LANGUAGE SQL") {
      return;
    }
    let bytes = body.as_bytes();
    let ubytes = upper.as_bytes();
    let n = bytes.len();
    let mut depth = 0i32;
    let mut first: Option<usize> = None;
    let mut i = 0;
    while i < n {
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
            i += 1;
          }
          if i < n {
            i += 1;
          }
          continue;
        },
        b'-' if i + 1 < n && bytes[i + 1] == b'-' => {
          // Line comment -- skip to end of line. Was matching WHERE
          // inside `-- WHERE foo` and flagging the next real WHERE
          // as a duplicate.
          while i < n && bytes[i] != b'\n' { i += 1 }
          continue;
        },
        _ => {},
      }
      if depth == 0 && i + 5 <= n && &upper[i..i + 5] == "WHERE" {
        let prev_ok = i == 0 || !is_word(ubytes[i - 1] as char);
        let next_ok = i + 5 == n || !is_word(ubytes[i + 5] as char);
        if prev_ok && next_ok {
          match first {
            None => {
              first = Some(i);
            },
            Some(prev_where) => {
              // A `UNION` / `INTERSECT` / `EXCEPT` between the previous
              // WHERE and this one means we're in separate set-op
              // branches -- each branch gets its own WHERE.
              let between = &upper[prev_where..i];
              if has_word_kw(between, "UNION")
                || has_word_kw(between, "INTERSECT")
                || has_word_kw(between, "EXCEPT")
              {
                first = Some(i);
                i += 5;
                continue;
              }
              let abs_start = start + i;
              let abs_end = start + i + 5;
              out.push(Diagnostic {
                code: "sql098",
                severity: Severity::Error,
                message: "duplicate top-level WHERE clause -- did you mean AND/OR?".into(),
                range: text_size::TextRange::new((abs_start as u32).into(), (abs_end as u32).into()),
              });
              return;
            },
          }
          i += 5;
          continue;
        }
      }
      i += 1;
    }
  }
}

fn is_word(c: char) -> bool {
  c.is_alphanumeric() || c == '_'
}

fn has_word_kw(haystack: &str, needle: &str) -> bool {
  let h = haystack.as_bytes();
  let n = needle.as_bytes();
  if n.is_empty() { return false; }
  let mut i = 0;
  while i + n.len() <= h.len() {
    if h[i..i + n.len()] == *n {
      let prev_ok = i == 0 || !is_word(h[i - 1] as char);
      let next_ok = i + n.len() == h.len() || !is_word(h[i + n.len()] as char);
      if prev_ok && next_ok { return true; }
    }
    i += 1;
  }
  false
}
