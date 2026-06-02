//! sql198: inline column CHECK references a different column.
//! e.g. `start_at DATE CHECK (end_at > start_at)`. PG raises 0A000
//! "cannot use column reference in DEFAULT/CHECK constraint" if the
//! CHECK is column-level (single inline constraint after column type).
//! Promote it to table-level CHECK instead.
//!
//! Conservative: only fires when the inline CHECK expression contains
//! an identifier that doesn't match the owning column.

use crate::{Diagnostic, LintRule, Severity};
use dsl_catalog::Catalog;
use dsl_parse::{Statement, StatementKind};
use dsl_resolve::Scope;

pub struct Rule;

impl LintRule for Rule {
  fn code(&self) -> &'static str {
    "sql198"
  }
  fn default_severity(&self) -> Severity {
    Severity::Warning
  }

  fn check(&self, source: &str, stmt: &Statement, _scope: &Scope, _catalog: &Catalog, out: &mut Vec<Diagnostic>) {
    let StatementKind::CreateTable(_) = &stmt.kind else { return };
    let (start, body, upper) = crate::stmt_body_upper(stmt, source);
    let Some(paren_at) = body.find('(') else { return };
    let Some(close_rel) = find_matching_paren(body, paren_at) else { return };
    let cols_text = &body[paren_at + 1..close_rel];
    let cols_upper = &upper[paren_at + 1..close_rel];
    for raw in split_top_level(cols_text) {
      let trimmed_upper = cols_upper[raw.start..raw.end].trim().to_ascii_uppercase();
      if trimmed_upper.starts_with("CONSTRAINT ")
        || trimmed_upper.starts_with("CHECK ")
        || trimmed_upper.starts_with("CHECK(")
        || trimmed_upper.starts_with("PRIMARY ")
        || trimmed_upper.starts_with("FOREIGN ")
        || trimmed_upper.starts_with("UNIQUE ")
        || trimmed_upper.starts_with("UNIQUE(")
        || trimmed_upper.starts_with("EXCLUDE ")
        || trimmed_upper.starts_with("LIKE ")
      {
        continue;
      }
      let frag = &cols_text[raw.start..raw.end];
      let frag_upper = &cols_upper[raw.start..raw.end];
      let Some(check_at) = frag_upper.find(" CHECK") else { continue };
      let post = &frag[check_at + " CHECK".len()..];
      let post_trim = post.trim_start();
      if !post_trim.starts_with('(') {
        continue;
      }
      let chk_open = check_at + " CHECK".len() + (post.len() - post_trim.len());
      let Some(chk_close_rel) = find_matching_paren(frag, chk_open) else { continue };
      let expr = &frag[chk_open + 1..chk_close_rel];
      let col_name = frag.split_whitespace().next().unwrap_or("").trim_matches('"');
      if col_name.is_empty() {
        continue;
      }
      let bytes = expr.as_bytes();
      let mut i = 0usize;
      let mut found_other = false;
      let mut other_name = String::new();
      while i < bytes.len() {
        if bytes[i] == b'\'' {
          // Skip string literal (handle '' escape).
          i += 1;
          while i < bytes.len() {
            if bytes[i] == b'\'' {
              if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                i += 2;
                continue;
              }
              i += 1;
              break;
            }
            i += 1;
          }
          continue;
        }
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
          let s = i;
          while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1
          }
          let id = &expr[s..i];
          // Skip if followed by `(` (function call name) -- not a column ref.
          let mut k = i;
          while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1
          }
          if k < bytes.len() && bytes[k] == b'(' {
            continue;
          }
          let up = id.to_ascii_uppercase();
          if !is_reserved(&up) && !id.eq_ignore_ascii_case(col_name) {
            found_other = true;
            other_name = id.to_string();
            break;
          }
        } else {
          i += 1;
        }
      }
      if !found_other {
        continue;
      }
      let abs_s = start + paren_at + 1 + raw.start;
      let abs_e = start + paren_at + 1 + raw.end;
      out.push(Diagnostic {
        code: "sql198",
        severity: Severity::Warning,
        message: format!(
          "Inline CHECK on `{col_name}` references `{other_name}` -- column-level CHECK can't reference other columns; promote to table-level CHECK"
        ),
        range: crate::range_at(abs_s, abs_e),
      });
    }
  }
}

struct Span {
  start: usize,
  end: usize,
}

fn split_top_level(text: &str) -> Vec<Span> {
  let mut out = Vec::new();
  let bytes = text.as_bytes();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut i = 0usize;
  while i < bytes.len() {
    match bytes[i] {
      b'(' => depth += 1,
      b')' => depth -= 1,
      b',' if depth == 0 => {
        out.push(Span { start, end: i });
        start = i + 1
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
  out.push(Span { start, end: bytes.len() });
  out
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

fn is_reserved(up: &str) -> bool {
  matches!(
    up,
    "AND"
      | "OR"
      | "NOT"
      | "IS"
      | "NULL"
      | "TRUE"
      | "FALSE"
      | "IN"
      | "BETWEEN"
      | "LIKE"
      | "ILIKE"
      | "ANY"
      | "ALL"
      | "SOME"
      | "EXISTS"
      | "DISTINCT"
      | "CASE"
      | "WHEN"
      | "THEN"
      | "ELSE"
      | "END"
      | "AS"
      | "ON"
      | "OFF"
      | "DEFAULT"
      | "CURRENT_DATE"
      | "CURRENT_TIME"
      | "CURRENT_TIMESTAMP"
      | "NOW"
      | "LOCALTIME"
      | "LOCALTIMESTAMP"
      | "CAST"
      | "USING"
  )
}
